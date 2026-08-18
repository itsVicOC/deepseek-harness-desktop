use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DesktopError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Keyring(#[from] keyring::Error),
    #[error("runtime failed: {0}")]
    Runtime(String),
    #[error("update source is not configured")]
    UpdateSourceNotConfigured,
    #[error("update signature is invalid")]
    InvalidSignature,
    #[error("update checksum is invalid")]
    InvalidChecksum,
    #[error("update is incompatible with this desktop version")]
    IncompatibleVersion,
    #[error("requested update was not found")]
    UpdateNotFound,
    #[error("Sparkle.framework is not available in this build")]
    SparkleUnavailable,
    #[error("archive contains an unsafe path")]
    UnsafeArchivePath,
    #[error("{0}")]
    Other(String),
}

impl DesktopError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "IO_ERROR",
            Self::Json(_) => "INVALID_JSON",
            Self::Http(_) => "NETWORK_ERROR",
            Self::Keyring(_) => "KEYCHAIN_ERROR",
            Self::Runtime(_) => "RUNTIME_ERROR",
            Self::UpdateSourceNotConfigured => "UPDATE_SOURCE_NOT_CONFIGURED",
            Self::InvalidSignature => "INVALID_SIGNATURE",
            Self::InvalidChecksum => "INVALID_CHECKSUM",
            Self::IncompatibleVersion => "INCOMPATIBLE_VERSION",
            Self::UpdateNotFound => "UPDATE_NOT_FOUND",
            Self::SparkleUnavailable => "SPARKLE_UNAVAILABLE",
            Self::UnsafeArchivePath => "UNSAFE_ARCHIVE_PATH",
            Self::Other(_) => "DESKTOP_ERROR",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl From<DesktopError> for CommandError {
    fn from(error: DesktopError) -> Self {
        Self {
            code: error.code().to_owned(),
            message: error.to_string(),
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
