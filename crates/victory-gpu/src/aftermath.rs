//! NVIDIA Nsight Aftermath GPU crash dumps (diagnostics builds, NVIDIA cards):
//! when the card faults or hangs ("device removed"), the driver writes what it
//! was running (the shader, the faulting address and the resource there) to a
//! dump, which goes next to the log as `aftermath-<pid>-<n>.nv-gpudmp`, is
//! turned into text with Nsight's `nv-aftermath-format` (`.txt` beside it),
//! and is summed up in the log line that ends the run ([`crate::host::fail`]).
//!
//! Enabled with VICTORY_AFTERMATH=1 when Nsight Graphics is installed (its SDK's library is found
//! under `C:\Program Files\NVIDIA Corporation\Nsight Graphics *`);
//! `VICTORY_AFTERMATH=0` leaves it off, `VICTORY_AFTERMATH_DLL` names the
//! library, `VICTORY_AFTERMATH_DIR` the folder for dumps (else the
//! capture folder, else `aftermath\` in the working folder).
//!
//! Nothing of Aftermath's is linked: the library is loaded at run time.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use windows::Win32::Graphics::Direct3D12::ID3D12Device;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{HSTRING, Interface, s};

/// `GFSDK_Aftermath_Version_API` of the SDK the declarations follow (2.27).
const VERSION: u32 = 0x21B;
const SUCCESS: u32 = 0x1;
/// `GFSDK_Aftermath_GpuCrashDumpWatchedApiFlags_DX`.
const WATCH_DX: u32 = 0x1;
/// `GFSDK_Aftermath_FeatureFlags_EnableResourceTracking`: the dump names the
/// resource at a faulting address (live or recently destroyed).
const RESOURCE_TRACKING: u32 = 0x2;
/// `GFSDK_Aftermath_FeatureFlags_EnableShaderErrorReporting`.
const SHADER_ERRORS: u32 = 0x10;
/// `GFSDK_Aftermath_CrashDump_Status_*`.
const STATUS_NOT_STARTED: u32 = 0;
const STATUS_FAILED: u32 = 2;
const STATUS_FINISHED: u32 = 4;

type CrashDumpCb = unsafe extern "C" fn(*const c_void, u32, *mut c_void);
type Enable = unsafe extern "C" fn(u32, u32, u32, Option<CrashDumpCb>, *const c_void, *const c_void, *const c_void, *mut c_void) -> u32;
type Dx12Initialize = unsafe extern "C" fn(u32, u32, *mut c_void) -> u32;
type Status = unsafe extern "C" fn(*mut u32) -> u32;

struct Lib {
    dx12_initialize: Dx12Initialize,
    status: Status,
}

static LIB: OnceLock<Option<Lib>> = OnceLock::new();
static DUMPS: AtomicU32 = AtomicU32::new(0);
/// The dumps written, with their text forms (if made).
static WRITTEN: std::sync::Mutex<Vec<(PathBuf, Option<PathBuf>)>> = std::sync::Mutex::new(Vec::new());

fn on() -> bool {
    crate::switch::on("VICTORY_AFTERMATH")
}

/// The newest Nsight Graphics install's folder.
fn nsight() -> Option<PathBuf> {
    let root = Path::new(r"C:\Program Files\NVIDIA Corporation");
    let mut installs: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("Nsight Graphics")))
        .collect();
    installs.sort();
    installs.pop()
}

fn library_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("VICTORY_AFTERMATH_DLL") {
        return Some(PathBuf::from(p));
    }
    let sdk = nsight()?.join("SDKs").join("NsightAftermathSDK");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(sdk).ok()?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    versions.sort();
    let dll = versions.pop()?.join(r"lib\x64\GFSDK_Aftermath_Lib.x64.dll");
    dll.exists().then_some(dll)
}

fn folder() -> PathBuf {
    if let Some(p) = std::env::var_os("VICTORY_AFTERMATH_DIR") {
        return PathBuf::from(p);
    }
    if let Some(p) = std::env::var_os("VICTORY_CAPTURE") {
        return PathBuf::from(p);
    }
    PathBuf::from("aftermath")
}

/// Called by Aftermath's thread with a finished dump.
unsafe extern "C" fn on_dump(data: *const c_void, size: u32, _user: *mut c_void) {
    if data.is_null() || size == 0 { return; }
    let n = DUMPS.fetch_add(1, Ordering::Relaxed);
    let dir = folder();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("aftermath-{}-{n}.nv-gpudmp", std::process::id()));
    // SAFETY: Aftermath passes a buffer of `size` bytes, valid during the call.
    let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size as usize) };
    if let Err(e) = std::fs::write(&path, bytes) {
        tracing::error!(error = %e, path = %path.display(), "Aftermath: the GPU crash dump could not be written");
        return;
    }
    let text = format_dump(&path);
    tracing::error!(dump = %path.display(), text = ?text.as_ref().map(|t| t.display().to_string()), "Aftermath: GPU crash dump written");
    WRITTEN.lock().unwrap_or_else(|e| e.into_inner()).push((path, text));
}

/// The dump in text, by Nsight's `nv-aftermath-format`, beside it.
fn format_dump(dump: &Path) -> Option<PathBuf> {
    let tool = nsight()?.join(r"host\windows-desktop-nomad-x64\nv-aftermath-format.exe");
    let out = std::process::Command::new(tool).arg(dump).output().ok()?;
    let text = dump.with_extension("txt");
    let mut all = out.stdout;
    all.extend_from_slice(&out.stderr);
    std::fs::write(&text, all).ok()?;
    Some(text)
}

/// Before the host device is made: enables crash dumps for D3D12 devices.
pub(crate) fn enable() {
    if !on() {
        return;
    }
    LIB.get_or_init(|| {
        let Some(path) = library_path() else {
            tracing::warn!("Aftermath SDK DLL was not found");
            return None;
        };
        // SAFETY: loading NVIDIA's library and looking up its documented exports, whose
        // signatures `Enable`, `Dx12Initialize` and `Status` follow (GFSDK_Aftermath.h,
        // GFSDK_Aftermath_GpuCrashDump.h; cdecl, which is the C ABI on x64).
        unsafe {
            let module = match LoadLibraryW(&HSTRING::from(path.as_os_str())) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(error = %e, path = %path.display(), "Aftermath: library not loaded");
                    return None;
                }
            };
            let enable: Enable = std::mem::transmute(GetProcAddress(module, s!("GFSDK_Aftermath_EnableGpuCrashDumps"))?);
            let dx12_initialize: Dx12Initialize = std::mem::transmute(GetProcAddress(module, s!("GFSDK_Aftermath_DX12_Initialize"))?);
            let status: Status = std::mem::transmute(GetProcAddress(module, s!("GFSDK_Aftermath_GetCrashDumpStatus"))?);
            let r = enable(VERSION, WATCH_DX, 0, Some(on_dump), std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null_mut());
            if r != SUCCESS {
                tracing::warn!(result = format!("{r:#x}"), "Aftermath: GPU crash dumps not enabled");
                return None;
            }
            tracing::info!(library = %path.display(), dumps = %folder().display(), "Aftermath: GPU crash dumps on");
            Some(Lib { dx12_initialize, status })
        }
    });
}

/// After the host device is made: resource tracking and shader error reporting on it
/// (the driver's own features; without them the dump still names the faulting shader).
pub(crate) fn device(device: &ID3D12Device) {
    let Some(Some(lib)) = LIB.get() else { return };
    // The driver can turn normally silent shader errors into device loss.
    // Request these traps only for an explicit validation run.
    let shader_errors = crate::switch::on("VICTORY_AFTERMATH_SHADER_ERRORS");
    let features = if shader_errors { RESOURCE_TRACKING | SHADER_ERRORS } else { RESOURCE_TRACKING };
    // SAFETY: a live device; the documented call.
    let r = unsafe { (lib.dx12_initialize)(VERSION, features, device.as_raw()) };
    if r != SUCCESS {
        // SAFETY: as above, without the optional features.
        let plain = unsafe { (lib.dx12_initialize)(VERSION, 0, device.as_raw()) };
        tracing::info!(with_features = format!("{r:#x}"), plain = format!("{plain:#x}"), "Aftermath: device features (0x1 is success)");
    } else {
        tracing::info!(shader_errors, "Aftermath: device watched, with resource tracking");
    }
}

/// When the device is lost: waits (at most 15 s) for the driver's dump, and returns
/// what to add to the failure's line: the dump's and its text's paths, and the
/// text's first lines.
pub(crate) fn report() -> Option<String> {
    let Some(Some(lib)) = LIB.get() else { return None };
    let start = std::time::Instant::now();
    loop {
        let mut status = STATUS_NOT_STARTED;
        // SAFETY: the documented call with an out value.
        unsafe { (lib.status)(&mut status) };
        let done = !WRITTEN.lock().unwrap_or_else(|e| e.into_inner()).is_empty();
        if done || status == STATUS_FAILED || status == STATUS_FINISHED || start.elapsed().as_secs() >= 15 {
            break;
        }
        // Not started after 3 s: the driver did not see a fault of its own (a hang or
        // removal it does not report), so no dump will come.
        if status == STATUS_NOT_STARTED && start.elapsed().as_secs() >= 3 {
            return Some("Aftermath: no GPU crash dump (the driver reported no fault)".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let written = WRITTEN.lock().unwrap_or_else(|e| e.into_inner());
    let Some((dump, text)) = written.last() else {
        return Some("Aftermath: the GPU crash dump was not collected".into());
    };
    let mut line = format!("Aftermath dump {}", dump.display());
    if let Some(text) = text {
        line.push_str(&format!(" (text {})", text.display()));
        if let Ok(all) = std::fs::read_to_string(text) {
            let head: Vec<&str> = all.lines().filter(|l| !l.trim().is_empty()).take(60).collect();
            line.push_str(&format!(":\n{}", head.join("\n")));
        }
    }
    Some(line)
}
