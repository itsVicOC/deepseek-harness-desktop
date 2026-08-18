use std::{fs, io::Write, path::Path};

use chrono::Utc;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use crate::{
    error::DesktopError,
    models::{DiagnosticsResult, RuntimeStatus},
    paths::DesktopPaths,
};

pub fn export(
    paths: &DesktopPaths,
    runtime: &RuntimeStatus,
) -> Result<DiagnosticsResult, DesktopError> {
    fs::create_dir_all(&paths.exports)?;
    let created_at = Utc::now();
    let filename = format!(
        "deepseek-harness-diagnostics-{}.zip",
        created_at.format("%Y%m%d-%H%M%S")
    );
    let target = paths.exports.join(filename);
    let file = fs::File::create(&target)?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o600);

    let mut redacted_runtime = runtime.clone();
    redacted_runtime.url = redacted_runtime.url.as_deref().map(redact_url);
    let status = serde_json::to_vec_pretty(&redacted_runtime)?;
    archive
        .start_file("runtime-status.json", options)
        .map_err(zip_error)?;
    archive.write_all(&status)?;

    for entry in fs::read_dir(&paths.logs)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        append_redacted_file(
            &mut archive,
            &entry.path(),
            &format!("logs/{name}"),
            options,
        )?;
    }
    archive.finish().map_err(zip_error)?;

    Ok(DiagnosticsResult {
        path: target.to_string_lossy().into_owned(),
        created_at: created_at.to_rfc3339(),
    })
}

pub fn clear_logs(paths: &DesktopPaths) -> Result<(), DesktopError> {
    for entry in fs::read_dir(&paths.logs)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            fs::OpenOptions::new()
                .write(true)
                .open(entry.path())?
                .set_len(0)?;
        }
    }
    Ok(())
}

fn append_redacted_file(
    archive: &mut ZipWriter<fs::File>,
    source: &Path,
    name: &str,
    options: SimpleFileOptions,
) -> Result<(), DesktopError> {
    if !source.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(source)?;
    archive.start_file(name, options).map_err(zip_error)?;
    archive.write_all(redact(&content).as_bytes())?;
    Ok(())
}

fn redact(content: &str) -> String {
    const SECRET_MARKERS: [&str; 5] = ["api_key", "apikey", "authorization", "password", "token"];
    content
        .lines()
        .map(|line| {
            let lowercase = line.to_ascii_lowercase();
            if SECRET_MARKERS
                .iter()
                .any(|marker| lowercase.contains(marker))
            {
                "[REDACTED]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_url(value: &str) -> String {
    url::Url::parse(value)
        .map(|mut url| {
            url.set_query(None);
            url.to_string()
        })
        .unwrap_or_else(|_| "[REDACTED]".into())
}

fn zip_error(error: zip::result::ZipError) -> DesktopError {
    DesktopError::Other(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{redact, redact_url};

    #[test]
    fn diagnostics_remove_secret_lines() {
        let output = redact("started\nAuthorization: Bearer secret\napi_key=secret\nready");
        assert_eq!(output, "started\n[REDACTED]\n[REDACTED]\nready");
        assert!(!output.contains("secret"));
    }

    #[test]
    fn diagnostics_remove_runtime_url_token() {
        assert_eq!(
            redact_url("http://127.0.0.1:4100/?dsh_token=secret"),
            "http://127.0.0.1:4100/"
        );
    }
}
