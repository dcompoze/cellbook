//! Image viewing utilities.

use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::errors::Result;

/// Open an image file in the configured viewer.
pub fn open_image(path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    println!("[image] {}", path.display());
    let viewer = get_image_viewer();
    spawn_viewer(&viewer, path)
}

/// Open image data in the configured viewer.
/// Writes the data to a temporary file with the given extension.
pub fn open_image_bytes(data: &[u8], extension: &str) -> Result<()> {
    let rand_id: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        ^ (std::process::id() as u64);

    let temp_path = std::env::temp_dir().join(format!("cellbook_{:x}.{}", rand_id, extension));

    let mut file = std::fs::File::create(&temp_path)?;
    file.write_all(data)?;
    file.flush()?;

    println!("[image] {}", temp_path.display());
    let viewer = get_image_viewer();
    spawn_viewer(&viewer, &temp_path)
}

/// Get the image viewer command.
/// Checks CELLBOOK_IMAGE_VIEWER env var, then falls back to platform default.
fn get_image_viewer() -> String {
    std::env::var("CELLBOOK_IMAGE_VIEWER").unwrap_or_else(|_| default_viewer().to_string())
}

/// Platform-specific default image viewer.
fn default_viewer() -> &'static str {
    if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "start"
    } else {
        "xdg-open"
    }
}

/// Spawn the viewer process.
fn spawn_viewer(viewer: &str, path: &Path) -> Result<()> {
    Command::new(viewer)
        .arg(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}
