use std::ffi::CString;

use tauri::AppHandle;
use tokio::sync::oneshot;

use crate::error::DesktopError;

unsafe extern "C" {
    fn dsh_sparkle_available() -> bool;
    fn dsh_sparkle_check_for_updates(feed_url: *const std::ffi::c_char) -> bool;
}

pub fn available() -> bool {
    unsafe { dsh_sparkle_available() }
}

pub async fn check_for_updates(app: &AppHandle, feed_url: &str) -> Result<(), DesktopError> {
    if !available() {
        return Err(DesktopError::SparkleUnavailable);
    }

    let feed_url = CString::new(feed_url)
        .map_err(|_| DesktopError::Other("Sparkle feed URL contains a null byte".into()))?;
    let (sender, receiver) = oneshot::channel();
    app.run_on_main_thread(move || {
        let result = unsafe { dsh_sparkle_check_for_updates(feed_url.as_ptr()) };
        let _ = sender.send(result);
    })
    .map_err(|error| DesktopError::Other(error.to_string()))?;

    if receiver
        .await
        .map_err(|error| DesktopError::Other(error.to_string()))?
    {
        Ok(())
    } else {
        Err(DesktopError::SparkleUnavailable)
    }
}
