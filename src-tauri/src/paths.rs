use std::{fs, path::PathBuf};

use crate::error::DesktopError;

#[derive(Debug, Clone)]
pub struct DesktopPaths {
    pub support: PathBuf,
    pub logs: PathBuf,
    pub runtime: PathBuf,
    pub exports: PathBuf,
    pub harness_home: PathBuf,
    pub bundled_runtime: Option<PathBuf>,
    pub source_checkout: Option<PathBuf>,
}

impl DesktopPaths {
    pub fn discover(resource_dir: Option<PathBuf>) -> Result<Self, DesktopError> {
        let support = dirs::data_dir()
            .ok_or_else(|| {
                DesktopError::Other("Application Support directory is unavailable".into())
            })?
            .join("DeepSeek Harness");
        let logs = support.join("logs");
        let runtime = support.join("runtime");
        let exports = support.join("exports");
        let harness_home = support.join("harness-home");

        for directory in [&support, &logs, &runtime, &exports, &harness_home] {
            fs::create_dir_all(directory)?;
        }

        let bundled_runtime = resource_dir
            .map(|dir| dir.join("runtime/current"))
            .filter(|dir| dir.exists());
        let source_checkout = std::env::current_dir()
            .ok()
            .and_then(|dir| dir.parent().map(|parent| parent.join("deepseek-harness")))
            .filter(|dir| dir.join("package.json").exists());

        Ok(Self {
            support,
            logs,
            runtime,
            exports,
            harness_home,
            bundled_runtime,
            source_checkout,
        })
    }
}
