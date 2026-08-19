use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    net::TcpListener,
    path::Path,
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use serde::Deserialize;
use tokio::{
    process::{Child, Command},
    sync::Mutex,
    time::sleep,
};
use uuid::Uuid;

use crate::{
    error::DesktopError,
    loopback_proxy::LoopbackProxy,
    models::{RuntimeState, RuntimeStatus},
    paths::DesktopPaths,
};

const START_TIMEOUT: Duration = Duration::from_secs(30);
const HEALTH_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundledManifest {
    runtime_version: String,
    upstream_commit: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimePointer {
    current_version: String,
    previous_version: Option<String>,
}

struct RuntimeInner {
    child: Option<Child>,
    proxy: Option<LoopbackProxy>,
    status: RuntimeStatus,
}

pub struct RuntimeManager {
    paths: DesktopPaths,
    client: reqwest::Client,
    operation: Arc<Mutex<()>>,
    inner: Mutex<RuntimeInner>,
}

impl RuntimeManager {
    pub fn new(paths: DesktopPaths, operation: Arc<Mutex<()>>) -> Self {
        let manifest = serde_json::from_str::<BundledManifest>(include_str!(
            "../../runtime/runtime-manifest.json"
        ))
        .expect("runtime/runtime-manifest.json must be valid");
        let rollback_available = read_pointer(&paths)
            .ok()
            .flatten()
            .and_then(|pointer| pointer.previous_version)
            .is_some();

        Self {
            paths,
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(10))
                .build()
                .expect("HTTP client configuration must be valid"),
            operation,
            inner: Mutex::new(RuntimeInner {
                child: None,
                proxy: None,
                status: RuntimeStatus {
                    state: RuntimeState::Stopped,
                    version: manifest.runtime_version,
                    upstream_commit: manifest.upstream_commit,
                    url: None,
                    pid: None,
                    last_error: None,
                    rollback_available,
                },
            }),
        }
    }

    pub async fn status(&self) -> RuntimeStatus {
        let mut inner = self.inner.lock().await;
        if let Ok(Some(pointer)) = read_pointer(&self.paths) {
            let root = self.paths.runtime.join(&pointer.current_version);
            if root.join("bin/dsh").is_file() {
                inner.status.version = pointer.current_version;
                if let Some(commit) = read_runtime_commit(&root) {
                    inner.status.upstream_commit = commit;
                }
                inner.status.rollback_available = pointer.previous_version.is_some();
            }
        }
        if let Some(child) = inner.child.as_mut() {
            match child.try_wait() {
                Ok(Some(exit)) => {
                    inner.child = None;
                    inner.proxy = None;
                    inner.status.pid = None;
                    inner.status.url = None;
                    if inner.status.state != RuntimeState::Stopping {
                        inner.status.state = RuntimeState::Failed;
                        inner.status.last_error = Some(format!("runtime exited with {exit}"));
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    inner.status.state = RuntimeState::Failed;
                    inner.status.last_error = Some(error.to_string());
                }
            }
        }
        inner.status.clone()
    }

    pub async fn start(&self, api_key: Option<String>) -> Result<RuntimeStatus, DesktopError> {
        let _operation = self.operation.lock().await;
        self.start_inner(api_key).await
    }

    pub(crate) async fn start_with_operation_held(
        &self,
        api_key: Option<String>,
    ) -> Result<RuntimeStatus, DesktopError> {
        self.start_inner(api_key).await
    }

    async fn start_inner(&self, api_key: Option<String>) -> Result<RuntimeStatus, DesktopError> {
        for attempt in 0..3 {
            match self.start_attempt(api_key.clone()).await {
                Ok(status) => return Ok(status),
                Err(error)
                    if attempt < 2
                        && matches!(&error, DesktopError::Runtime(message) if message.contains("did not become healthy") || message.contains("exited during startup")) =>
                {
                    tracing::debug!(
                        attempt = attempt + 1,
                        "runtime startup failed; retrying with a fresh port"
                    );
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("runtime startup retry loop always returns")
    }

    async fn start_attempt(&self, api_key: Option<String>) -> Result<RuntimeStatus, DesktopError> {
        {
            let inner = self.inner.lock().await;
            if inner.child.is_some() {
                return Ok(inner.status.clone());
            }
        }

        let upstream_port = reserve_loopback_port()?;
        let upstream_url = format!("http://127.0.0.1:{upstream_port}");
        let (mut command, version, upstream_commit) = self.runtime_command(upstream_port)?;
        let rollback_available = read_pointer(&self.paths)?
            .and_then(|pointer| pointer.previous_version)
            .is_some();
        self.configure_environment(&mut command, api_key);

        let log_path = self.paths.logs.join("runtime.log");
        let stdout = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let stderr = stdout.try_clone()?;
        command
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .kill_on_drop(true);

        {
            let mut inner = self.inner.lock().await;
            inner.status.state = RuntimeState::Starting;
            inner.status.last_error = None;
        }

        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let mut inner = self.inner.lock().await;
                inner.status.state = RuntimeState::Failed;
                inner.status.last_error = Some(format!("failed to start dsh: {error}"));
                return Err(DesktopError::Runtime(format!(
                    "failed to start dsh: {error}"
                )));
            }
        };
        let pid = child.id();

        {
            let mut inner = self.inner.lock().await;
            inner.child = Some(child);
            inner.status.version = version;
            inner.status.upstream_commit = upstream_commit;
            inner.status.rollback_available = rollback_available;
            inner.status.pid = pid;
            inner.status.url = None;
        }

        match self.wait_until_healthy(&upstream_url).await {
            Ok(()) => {
                let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
                let proxy = match LoopbackProxy::start(upstream_port, token.clone()).await {
                    Ok(proxy) => proxy,
                    Err(error) => {
                        let mut child = {
                            let mut inner = self.inner.lock().await;
                            inner.child.take()
                        };
                        if let Some(child) = child.as_mut() {
                            let _ = child.kill().await;
                            let _ = child.wait().await;
                        }
                        let mut inner = self.inner.lock().await;
                        inner.status.state = RuntimeState::Failed;
                        inner.status.pid = None;
                        inner.status.last_error = Some(error.to_string());
                        return Err(error);
                    }
                };
                let url = format!("http://127.0.0.1:{}/?dsh_token={token}", proxy.port);
                let mut inner = self.inner.lock().await;
                if inner.child.is_none() {
                    return Err(DesktopError::Runtime(
                        "runtime stopped during startup".into(),
                    ));
                }
                inner.proxy = Some(proxy);
                inner.status.url = Some(url);
                inner.status.state = RuntimeState::Running;
                Ok(inner.status.clone())
            }
            Err(error) => {
                let mut child = {
                    let mut inner = self.inner.lock().await;
                    inner.child.take()
                };
                if let Some(child) = child.as_mut() {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
                let mut inner = self.inner.lock().await;
                inner.status.state = RuntimeState::Failed;
                inner.status.url = None;
                inner.status.pid = None;
                inner.status.last_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    pub async fn stop(&self) -> Result<RuntimeStatus, DesktopError> {
        let _operation = self.operation.lock().await;
        self.stop_inner().await
    }

    pub(crate) async fn stop_with_operation_held(&self) -> Result<RuntimeStatus, DesktopError> {
        self.stop_inner().await
    }

    async fn stop_inner(&self) -> Result<RuntimeStatus, DesktopError> {
        let mut child = {
            let mut inner = self.inner.lock().await;
            inner.status.state = RuntimeState::Stopping;
            inner.proxy = None;
            inner.child.take()
        };

        if let Some(child) = child.as_mut() {
            child.kill().await?;
            child.wait().await?;
        }

        let mut inner = self.inner.lock().await;
        inner.status.state = RuntimeState::Stopped;
        inner.status.url = None;
        inner.status.pid = None;
        inner.status.last_error = None;
        Ok(inner.status.clone())
    }

    pub async fn restart(&self, api_key: Option<String>) -> Result<RuntimeStatus, DesktopError> {
        let _operation = self.operation.lock().await;
        self.stop_inner().await?;
        self.start_inner(api_key).await
    }

    fn runtime_command(&self, port: u16) -> Result<(Command, String, String), DesktopError> {
        let bundled = serde_json::from_str::<BundledManifest>(include_str!(
            "../../runtime/runtime-manifest.json"
        ))?;
        if let Some(binary) = std::env::var("DSH_DESKTOP_DSH_BIN")
            .ok()
            .filter(|value| !value.is_empty())
        {
            let mut command = Command::new(binary);
            command.args(["web", "--host", "127.0.0.1", "--port", &port.to_string()]);
            return Ok((command, bundled.runtime_version, bundled.upstream_commit));
        }

        if let Some(pointer) = read_pointer(&self.paths)? {
            let root = self.paths.runtime.join(&pointer.current_version);
            if let Some(command) = packaged_command(&root, port) {
                let commit =
                    read_runtime_commit(&root).unwrap_or_else(|| bundled.upstream_commit.clone());
                return Ok((command, pointer.current_version, commit));
            }
        }

        if let Some(root) = self.paths.bundled_runtime.as_ref() {
            if let Some(command) = packaged_command(root, port) {
                return Ok((command, bundled.runtime_version, bundled.upstream_commit));
            }
        }

        if let Some(source) = self.paths.source_checkout.as_ref() {
            let mut command = Command::new("pnpm");
            command.arg("--dir").arg(source).args([
                "dsh",
                "web",
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
            ]);
            return Ok((command, bundled.runtime_version, bundled.upstream_commit));
        }

        Err(DesktopError::Runtime(
            "no bundled, installed, or source runtime is available".into(),
        ))
    }

    fn configure_environment(&self, command: &mut Command, api_key: Option<String>) {
        let inherited: HashMap<String, String> =
            ["PATH", "HOME", "TMPDIR", "LANG", "SHELL", "TERM"]
                .into_iter()
                .filter_map(|key| std::env::var(key).ok().map(|value| (key.to_owned(), value)))
                .collect();
        command
            .env_clear()
            .envs(inherited)
            .env("DSH_HOME", &self.paths.harness_home)
            .current_dir(&self.paths.support);
        if let Some(api_key) = api_key.filter(|value| !value.is_empty()) {
            command.env("DEEPSEEK_API_KEY", api_key);
        }
    }

    async fn wait_until_healthy(&self, url: &str) -> Result<(), DesktopError> {
        let started = tokio::time::Instant::now();
        while started.elapsed() < START_TIMEOUT {
            if let Ok(response) = self.client.get(url).send().await {
                if response.status().is_success() {
                    return Ok(());
                }
            }

            let mut inner = self.inner.lock().await;
            if let Some(child) = inner.child.as_mut() {
                if let Some(status) = child.try_wait()? {
                    return Err(DesktopError::Runtime(format!(
                        "runtime exited during startup with {status}"
                    )));
                }
            } else {
                return Err(DesktopError::Runtime(
                    "runtime stopped during startup".into(),
                ));
            }
            drop(inner);
            sleep(HEALTH_INTERVAL).await;
        }
        Err(DesktopError::Runtime(format!(
            "runtime did not become healthy within {} seconds",
            START_TIMEOUT.as_secs()
        )))
    }
}

fn reserve_loopback_port() -> Result<u16, DesktopError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    Ok(listener.local_addr()?.port())
}

fn packaged_command(root: &Path, port: u16) -> Option<Command> {
    let launcher = root.join("bin/dsh");
    if !launcher.is_file() {
        return None;
    }
    let mut command = Command::new(launcher);
    command.args(["web", "--host", "127.0.0.1", "--port", &port.to_string()]);
    Some(command)
}

fn read_runtime_commit(root: &Path) -> Option<String> {
    let content = fs::read_to_string(root.join("runtime.json")).ok()?;
    let manifest = serde_json::from_str::<BundledManifest>(&content).ok()?;
    Some(manifest.upstream_commit)
}

fn read_pointer(paths: &DesktopPaths) -> Result<Option<RuntimePointer>, DesktopError> {
    let path = paths.runtime.join("current.json");
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
}

#[cfg(test)]
mod tests {
    use super::reserve_loopback_port;
    use std::{io::ErrorKind, net::TcpListener};

    #[test]
    fn reserved_port_is_loopback_bindable() {
        let port = match reserve_loopback_port() {
            Ok(port) => port,
            Err(crate::error::DesktopError::Io(error))
                if error.kind() == ErrorKind::PermissionDenied =>
            {
                return;
            }
            Err(error) => panic!("reserve port: {error}"),
        };
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind released port");
        assert_eq!(
            listener
                .local_addr()
                .expect("local address")
                .ip()
                .to_string(),
            "127.0.0.1"
        );
    }
}
