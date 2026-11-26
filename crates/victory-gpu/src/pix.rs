//! PIX GPU captures of the host's D3D12 stream: with `VICTORY_PIX=1` the
//! newest installed `WinPixGpuCapturer.dll` (under `C:\Program Files\Microsoft
//! PIX\<version>`, or `VICTORY_PIX_DLL`) is loaded before the host device is
//! made, so PIX can attach to the running process and take a GPU capture of
//! a frame (Attach, then GPU Capture). PIX requires the capturer to be loaded
//! before the first device; loading it later does nothing.
//!
//! A capture shows every draw with its bound views, so a wrongly textured
//! draw (an NPC's skin, a car's paint) can be followed to the texture it
//! read. Capturing slows the frame and holds a copy of the frame's memory.

use std::path::{Path, PathBuf};

use windows::Win32::System::LibraryLoader::LoadLibraryW;
use windows::core::HSTRING;

fn library_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("VICTORY_PIX_DLL") {
        return Some(PathBuf::from(p));
    }
    let mut versions: Vec<PathBuf> = std::fs::read_dir(Path::new(r"C:\Program Files\Microsoft PIX"))
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("WinPixGpuCapturer.dll").exists())
        .collect();
    versions.sort();
    versions.pop().map(|v| v.join("WinPixGpuCapturer.dll"))
}

/// Loads the PIX GPU capturer if `VICTORY_PIX=1`: call before the host
/// device is made.
pub(crate) fn load() {
    if !crate::switch::on("VICTORY_PIX") {
        return;
    }
    let Some(path) = library_path() else {
        tracing::warn!("PIX: WinPixGpuCapturer.dll was not found");
        return;
    };
    // SAFETY: loading Microsoft's capture library, which hooks D3D12 when
    // loaded; nothing of it is called directly.
    match unsafe { LoadLibraryW(&HSTRING::from(path.as_os_str())) } {
        Ok(_) => tracing::info!(path = %path.display(), "PIX GPU capturer loaded: attach PIX to this process to capture"),
        Err(e) => tracing::warn!(error = %e, path = %path.display(), "PIX: capturer not loaded"),
    }
}
