//! Maintainer smoke fixture. No game content, user ROM, audio or microphone.
#[cfg(any(target_os = "linux", target_os = "windows"))]
include!("service_window_supported.rs");

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("the native service-window fixture requires Linux or Windows")
}
