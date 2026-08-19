use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use flate2::read::GzDecoder;
use quick_xml::{events::Event, Reader};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::{Builder, NamedTempFile};
use tokio::{io::AsyncWriteExt, time::Duration};
use url::Url;

use crate::{
    error::DesktopError,
    models::{UpdateChannel, UpdatePhase, UpdateStatus},
    paths::DesktopPaths,
};

const DEFAULT_RUNTIME_STABLE: &str = "https://github.com/itsVicOC/deepseek-harness-desktop/releases/download/runtime-stable/runtime-stable.json";
const DEFAULT_RUNTIME_BETA: &str =
    "https://github.com/itsVicOC/deepseek-harness-desktop/releases/download/runtime-beta/runtime-beta.json";
const DEFAULT_APPCAST_STABLE: &str = "https://github.com/itsVicOC/deepseek-harness-desktop/releases/download/desktop-stable/appcast-stable.xml";
const DEFAULT_APPCAST_BETA: &str =
    "https://github.com/itsVicOC/deepseek-harness-desktop/releases/download/desktop-beta/appcast-beta.xml";
const MAX_RUNTIME_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_APPCAST_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRelease {
    pub version: String,
    pub upstream_commit: String,
    pub archive_url: String,
    pub sha256: String,
    pub desktop_min_version: String,
    pub desktop_max_version: String,
    pub release_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedRuntimeManifest {
    pub schema_version: u32,
    pub payload: RuntimeRelease,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimePointer {
    current_version: String,
    previous_version: Option<String>,
}

#[derive(Debug)]
struct AppRelease {
    version: String,
    archive_url: String,
    release_notes: Option<String>,
}

pub struct UpdateManager {
    paths: DesktopPaths,
    desktop_version: Version,
    client: reqwest::Client,
}

impl UpdateManager {
    pub fn new(paths: DesktopPaths) -> Self {
        Self {
            paths,
            desktop_version: Version::parse(env!("CARGO_PKG_VERSION"))
                .expect("package version must be semver"),
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .timeout(Duration::from_secs(120))
                .build()
                .expect("HTTP client configuration must be valid"),
        }
    }

    pub async fn check_runtime(
        &self,
        channel: UpdateChannel,
        current_version: &str,
        rollback_available: bool,
    ) -> Result<UpdateStatus, DesktopError> {
        let manifest = self.fetch_runtime_manifest(channel).await?;
        self.verify_compatibility(&manifest.payload)?;
        let current = Version::parse(current_version).map_err(|error| {
            DesktopError::Other(format!("invalid current runtime version: {error}"))
        })?;
        let available = Version::parse(&manifest.payload.version).map_err(|error| {
            DesktopError::Other(format!("invalid release runtime version: {error}"))
        })?;
        let has_update = available > current;

        Ok(UpdateStatus {
            component: "runtime".into(),
            current_version: current_version.into(),
            available_version: has_update.then_some(manifest.payload.version),
            channel,
            phase: if has_update {
                UpdatePhase::Available
            } else {
                UpdatePhase::Current
            },
            progress: 0,
            requires_restart: false,
            error_code: None,
            rollback_available,
            release_notes: manifest.payload.release_notes,
        })
    }

    pub(crate) async fn install_runtime_with_operation_held(
        &self,
        channel: UpdateChannel,
        requested_version: &str,
        current_version: &str,
    ) -> Result<UpdateStatus, DesktopError> {
        self.install_runtime_inner(channel, requested_version, current_version)
            .await
    }

    async fn install_runtime_inner(
        &self,
        channel: UpdateChannel,
        requested_version: &str,
        current_version: &str,
    ) -> Result<UpdateStatus, DesktopError> {
        let manifest = self.fetch_runtime_manifest(channel).await?;
        self.verify_compatibility(&manifest.payload)?;
        if manifest.payload.version != requested_version {
            return Err(DesktopError::UpdateNotFound);
        }
        validate_newer_version(current_version, &manifest.payload.version)?;
        validate_https_url(&manifest.payload.archive_url)?;

        let archive = self
            .download_runtime_archive(&manifest.payload.archive_url, &manifest.payload.sha256)
            .await?;
        self.install_archive(archive.path(), &manifest.payload)?;

        Ok(UpdateStatus {
            component: "runtime".into(),
            current_version: manifest.payload.version,
            available_version: None,
            channel,
            phase: UpdatePhase::Current,
            progress: 100,
            requires_restart: false,
            error_code: None,
            rollback_available: true,
            release_notes: manifest.payload.release_notes,
        })
    }

    pub async fn check_app(&self, channel: UpdateChannel) -> Result<UpdateStatus, DesktopError> {
        let endpoint = appcast_endpoint(channel);
        validate_https_url(&endpoint)?;
        let response = self.client.get(endpoint).send().await?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_APPCAST_BYTES)
        {
            return Err(DesktopError::Other("appcast is too large".into()));
        }
        let xml = response.bytes().await?;
        if xml.len() as u64 > MAX_APPCAST_BYTES {
            return Err(DesktopError::Other("appcast is too large".into()));
        }
        let xml = std::str::from_utf8(&xml)
            .map_err(|error| DesktopError::Other(format!("invalid appcast encoding: {error}")))?;
        let release = parse_appcast(&xml)?;
        validate_https_url(&release.archive_url)?;
        let available = Version::parse(&release.version)
            .map_err(|error| DesktopError::Other(format!("invalid appcast version: {error}")))?;
        let has_update = available > self.desktop_version;

        Ok(UpdateStatus {
            component: "desktop".into(),
            current_version: self.desktop_version.to_string(),
            available_version: has_update.then_some(release.version),
            channel,
            phase: if has_update {
                UpdatePhase::Available
            } else {
                UpdatePhase::Current
            },
            progress: 0,
            requires_restart: true,
            error_code: None,
            rollback_available: false,
            release_notes: release.release_notes,
        })
    }

    pub fn appcast_url(&self, channel: UpdateChannel) -> String {
        appcast_endpoint(channel)
    }

    pub(crate) async fn rollback_runtime_with_operation_held(&self) -> Result<(), DesktopError> {
        self.rollback_runtime_inner()
    }

    fn rollback_runtime_inner(&self) -> Result<(), DesktopError> {
        let pointer_path = self.paths.runtime.join("current.json");
        let mut pointer: RuntimePointer =
            serde_json::from_str(&fs::read_to_string(&pointer_path)?)?;
        let previous = pointer
            .previous_version
            .take()
            .ok_or(DesktopError::UpdateNotFound)?;
        pointer.previous_version = Some(std::mem::replace(&mut pointer.current_version, previous));
        write_pointer(&pointer_path, &pointer)
    }

    async fn fetch_runtime_manifest(
        &self,
        channel: UpdateChannel,
    ) -> Result<SignedRuntimeManifest, DesktopError> {
        let public_key = runtime_public_key()?;
        let endpoint = runtime_endpoint(channel);
        validate_https_url(&endpoint)?;
        let manifest = self
            .client
            .get(endpoint)
            .send()
            .await?
            .error_for_status()?
            .json::<SignedRuntimeManifest>()
            .await?;
        verify_manifest(&manifest, &public_key)?;
        Ok(manifest)
    }

    fn verify_compatibility(&self, release: &RuntimeRelease) -> Result<(), DesktopError> {
        let minimum = Version::parse(&release.desktop_min_version)
            .map_err(|error| DesktopError::Other(format!("invalid desktopMinVersion: {error}")))?;
        let maximum = Version::parse(&release.desktop_max_version)
            .map_err(|error| DesktopError::Other(format!("invalid desktopMaxVersion: {error}")))?;
        if self.desktop_version < minimum || self.desktop_version > maximum {
            return Err(DesktopError::IncompatibleVersion);
        }
        Ok(())
    }

    async fn download_runtime_archive(
        &self,
        url: &str,
        expected_sha256: &str,
    ) -> Result<NamedTempFile, DesktopError> {
        fs::create_dir_all(&self.paths.runtime)?;
        let mut response = self.client.get(url).send().await?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RUNTIME_ARCHIVE_BYTES)
        {
            return Err(DesktopError::Other("runtime archive is too large".into()));
        }

        let temporary = Builder::new()
            .prefix("runtime-download-")
            .tempfile_in(&self.paths.runtime)?;
        let mut output = tokio::fs::File::from_std(temporary.reopen()?);
        let mut digest = Sha256::new();
        let mut total = 0_u64;
        while let Some(chunk) = response.chunk().await? {
            total = total
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| DesktopError::Other("runtime archive is too large".into()))?;
            if total > MAX_RUNTIME_ARCHIVE_BYTES {
                return Err(DesktopError::Other("runtime archive is too large".into()));
            }
            digest.update(&chunk);
            output.write_all(&chunk).await?;
        }
        output.flush().await?;
        output.sync_all().await?;
        verify_checksum(&hex::encode(digest.finalize()), expected_sha256)?;
        Ok(temporary)
    }

    fn install_archive(
        &self,
        archive_path: &Path,
        release: &RuntimeRelease,
    ) -> Result<(), DesktopError> {
        fs::create_dir_all(&self.paths.runtime)?;
        let staging = Builder::new()
            .prefix("runtime-update-")
            .tempdir_in(&self.paths.runtime)?;
        unpack_archive(archive_path, staging.path())?;
        let launcher = staging.path().join("bin/dsh");
        if !launcher.is_file() {
            return Err(DesktopError::Other(
                "runtime archive is missing bin/dsh".into(),
            ));
        }
        fs::write(
            staging.path().join("runtime.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "runtimeVersion": release.version,
                "upstreamCommit": release.upstream_commit,
            }))?,
        )?;

        let target = self.paths.runtime.join(&release.version);
        if target.exists() {
            let existing_launcher = target.join("bin/dsh");
            if !existing_launcher.is_file() {
                fs::remove_dir_all(&target)?;
            } else {
                return Err(DesktopError::Other(
                    "runtime version is already installed".into(),
                ));
            }
        }
        let staging_path = staging.keep();
        fs::rename(&staging_path, &target)?;

        let pointer_path = self.paths.runtime.join("current.json");
        let previous = read_pointer(&pointer_path)?.map(|pointer| pointer.current_version);
        write_pointer(
            &pointer_path,
            &RuntimePointer {
                current_version: release.version.clone(),
                previous_version: previous.filter(|version| version != &release.version),
            },
        )
    }
}

fn runtime_endpoint(channel: UpdateChannel) -> String {
    let key = match channel {
        UpdateChannel::Stable => "DSH_RUNTIME_STABLE_URL",
        UpdateChannel::Beta => "DSH_RUNTIME_BETA_URL",
    };
    debug_override(key).unwrap_or_else(|| match channel {
        UpdateChannel::Stable => DEFAULT_RUNTIME_STABLE.into(),
        UpdateChannel::Beta => DEFAULT_RUNTIME_BETA.into(),
    })
}

fn appcast_endpoint(channel: UpdateChannel) -> String {
    let key = match channel {
        UpdateChannel::Stable => "DSH_APPCAST_STABLE_URL",
        UpdateChannel::Beta => "DSH_APPCAST_BETA_URL",
    };
    debug_override(key).unwrap_or_else(|| match channel {
        UpdateChannel::Stable => DEFAULT_APPCAST_STABLE.into(),
        UpdateChannel::Beta => DEFAULT_APPCAST_BETA.into(),
    })
}

fn runtime_public_key() -> Result<Vec<u8>, DesktopError> {
    let encoded = debug_override("DSH_RUNTIME_PUBLIC_KEY").unwrap_or_else(|| {
        include_str!("../../runtime/public-key.txt")
            .trim()
            .to_owned()
    });
    if encoded.is_empty() || encoded.starts_with("REPLACE_") {
        return Err(DesktopError::UpdateSourceNotConfigured);
    }
    BASE64
        .decode(encoded)
        .map_err(|_| DesktopError::InvalidSignature)
}

#[cfg(debug_assertions)]
fn debug_override(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

#[cfg(not(debug_assertions))]
fn debug_override(_key: &str) -> Option<String> {
    None
}

pub fn verify_manifest(
    manifest: &SignedRuntimeManifest,
    public_key: &[u8],
) -> Result<(), DesktopError> {
    if manifest.schema_version != 1 {
        return Err(DesktopError::InvalidSignature);
    }
    let key_bytes: [u8; 32] = public_key
        .try_into()
        .map_err(|_| DesktopError::InvalidSignature)?;
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| DesktopError::InvalidSignature)?;
    let signature_bytes = BASE64
        .decode(&manifest.signature)
        .map_err(|_| DesktopError::InvalidSignature)?;
    let signature =
        Signature::from_slice(&signature_bytes).map_err(|_| DesktopError::InvalidSignature)?;
    let canonical = serde_json::to_vec(&manifest.payload)?;
    key.verify(&canonical, &signature)
        .map_err(|_| DesktopError::InvalidSignature)
}

fn verify_checksum(actual: &str, expected: &str) -> Result<(), DesktopError> {
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(DesktopError::InvalidChecksum);
    }
    Ok(())
}

fn validate_newer_version(current: &str, requested: &str) -> Result<(), DesktopError> {
    let current = Version::parse(current).map_err(|error| {
        DesktopError::Other(format!("invalid current runtime version: {error}"))
    })?;
    let requested = Version::parse(requested).map_err(|error| {
        DesktopError::Other(format!("invalid release runtime version: {error}"))
    })?;
    if requested <= current {
        return Err(DesktopError::Other(
            "runtime installation only accepts a newer version; use rollback for downgrade".into(),
        ));
    }
    Ok(())
}

fn validate_https_url(value: &str) -> Result<(), DesktopError> {
    let url = Url::parse(value)
        .map_err(|error| DesktopError::Other(format!("invalid update URL: {error}")))?;
    if url.scheme() != "https" || url.host_str().is_none() {
        return Err(DesktopError::Other("update URL must use HTTPS".into()));
    }
    Ok(())
}

fn unpack_archive(archive_path: &Path, destination: &Path) -> Result<(), DesktopError> {
    let decoder = GzDecoder::new(fs::File::open(archive_path)?);
    let mut archive = tar::Archive::new(decoder);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(DesktopError::UnsafeArchivePath);
        }
        if !entry.unpack_in(destination)? {
            return Err(DesktopError::UnsafeArchivePath);
        }
    }
    Ok(())
}

fn read_pointer(path: &Path) -> Result<Option<RuntimePointer>, DesktopError> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
}

fn write_pointer(path: &Path, pointer: &RuntimePointer) -> Result<(), DesktopError> {
    let temporary = PathBuf::from(format!("{}.{}.tmp", path.display(), std::process::id()));
    let mut file = fs::File::create(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(pointer)?)?;
    file.sync_all()?;
    fs::rename(temporary, path)?;
    if let Some(parent) = path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

fn parse_appcast(xml: &str) -> Result<AppRelease, DesktopError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut inside_item = false;
    let mut inside_description = false;
    let mut version = None;
    let mut archive_url = None;
    let mut release_notes = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) if element.local_name().as_ref() == b"item" => {
                inside_item = true
            }
            Ok(Event::End(element)) if element.local_name().as_ref() == b"item" => break,
            Ok(Event::Start(element))
                if inside_item && element.local_name().as_ref() == b"description" =>
            {
                inside_description = true;
            }
            Ok(Event::End(element)) if element.local_name().as_ref() == b"description" => {
                inside_description = false
            }
            Ok(Event::Text(text)) if inside_description => {
                release_notes = Some(
                    text.unescape()
                        .map_err(|error| DesktopError::Other(error.to_string()))?
                        .into_owned(),
                );
            }
            Ok(Event::Empty(element)) | Ok(Event::Start(element))
                if inside_item && element.local_name().as_ref() == b"enclosure" =>
            {
                for attribute in element.attributes().with_checks(false) {
                    let attribute =
                        attribute.map_err(|error| DesktopError::Other(error.to_string()))?;
                    let local = attribute.key.local_name();
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|error| DesktopError::Other(error.to_string()))?
                        .into_owned();
                    match local.as_ref() {
                        b"version" => version = Some(value),
                        b"url" => archive_url = Some(value),
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => return Err(DesktopError::Other(format!("invalid appcast: {error}"))),
            _ => {}
        }
    }

    Ok(AppRelease {
        version: version
            .ok_or_else(|| DesktopError::Other("appcast item is missing sparkle:version".into()))?,
        archive_url: archive_url
            .ok_or_else(|| DesktopError::Other("appcast item is missing enclosure URL".into()))?,
        release_notes,
    })
}

#[cfg(test)]
mod tests {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use ed25519_dalek::{Signer, SigningKey};
    use flate2::{write::GzEncoder, Compression};
    use sha2::Digest;
    use tar::Builder;
    use tempfile::tempdir;

    use super::{
        parse_appcast, unpack_archive, validate_newer_version, verify_checksum, verify_manifest,
        RuntimeRelease, SignedRuntimeManifest,
    };

    #[test]
    fn verifies_signed_runtime_manifest() {
        let signing = SigningKey::from_bytes(&[7_u8; 32]);
        let payload = RuntimeRelease {
            version: "0.1.1".into(),
            upstream_commit: "abc123".into(),
            archive_url: "https://example.com/runtime.tar.gz".into(),
            sha256: "00".repeat(32),
            desktop_min_version: "0.1.0".into(),
            desktop_max_version: "0.2.0".into(),
            release_notes: None,
        };
        let signature = signing.sign(&serde_json::to_vec(&payload).expect("serialize payload"));
        let manifest = SignedRuntimeManifest {
            schema_version: 1,
            payload,
            signature: BASE64.encode(signature.to_bytes()),
        };
        assert!(verify_manifest(&manifest, signing.verifying_key().as_bytes()).is_ok());

        let mut tampered = manifest;
        tampered.payload.version = "9.9.9".into();
        assert!(verify_manifest(&tampered, signing.verifying_key().as_bytes()).is_err());
    }

    #[test]
    fn rejects_checksum_mismatch() {
        let actual = hex::encode(sha2::Sha256::digest(b"runtime"));
        assert!(verify_checksum(&actual, &actual).is_ok());
        assert!(verify_checksum(&actual, &"00".repeat(32)).is_err());
    }

    #[test]
    fn runtime_install_requires_a_newer_version() {
        assert!(validate_newer_version("0.1.0", "0.1.1").is_ok());
        assert!(validate_newer_version("0.1.1", "0.1.1").is_err());
        assert!(validate_newer_version("0.1.1", "0.1.0").is_err());
    }

    #[test]
    fn parses_sparkle_appcast_item() {
        let xml = r#"<rss xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle"><channel><item><description>Fixes</description><enclosure url="https://example.com/app.zip" sparkle:version="0.2.0" /></item></channel></rss>"#;
        let release = parse_appcast(xml).expect("parse appcast");
        assert_eq!(release.version, "0.2.0");
        assert_eq!(release.archive_url, "https://example.com/app.zip");
        assert_eq!(release.release_notes.as_deref(), Some("Fixes"));
    }

    #[test]
    fn rejects_archive_path_traversal() {
        let root = tempdir().expect("temp root");
        let archive_path = root.path().join("payload.tar.gz");
        let archive_file = std::fs::File::create(&archive_path).expect("archive");
        let encoder = GzEncoder::new(archive_file, Compression::default());
        let mut builder = Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_path("safe").expect("path");
        header.as_mut_bytes()[..10].copy_from_slice(b"../escape\0");
        header.set_size(4);
        header.set_cksum();
        builder.append(&header, &b"test"[..]).expect("entry");
        builder
            .into_inner()
            .expect("finish gzip")
            .finish()
            .expect("finish archive");

        let destination = root.path().join("destination");
        std::fs::create_dir(&destination).expect("destination");
        assert!(unpack_archive(&archive_path, &destination).is_err());
        assert!(!root.path().join("escape").exists());
    }
}
