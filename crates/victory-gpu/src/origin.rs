//! What lies at the addresses shaders are given outside the title's buffer
//! objects (`shadow::loose`): every such address becomes a window, a copy of
//! that memory the card reads. On the console any title address is one the
//! card may read; here the same process also holds this layer's own memory,
//! the system's modules and memory nobody committed, so an address taken for
//! the card's that is none of the title's shows here.
//!
//! Each new window's range is walked region by region (`VirtualQuery`) and
//! its bytes are counted by kind: not committed, a module's image (named),
//! private memory and mapped memory, and whether below 4 GiB (the title's
//! graphics memory lies above it unless it asked for 32-bit addresses:
//! `victory-hle` xmem `address_range`). The first windows are said one by
//! one; the totals by kind with every one.

use std::collections::BTreeMap;
use std::sync::Mutex;

use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows_sys::Win32::System::Memory::{MEMORY_BASIC_INFORMATION, VirtualQuery};

const MEM_COMMIT: u32 = 0x1000;
const MEM_IMAGE: u32 = 0x100_0000;
const MEM_MAPPED: u32 = 0x4_0000;
const MEM_PRIVATE: u32 = 0x2_0000;

/// Bytes of windows by kind, since the start.
static TOTALS: Mutex<BTreeMap<String, (u64, u64)>> = Mutex::new(BTreeMap::new());

fn module_name(base: *mut core::ffi::c_void) -> String {
    let mut name = [0u16; 260];
    // SAFETY: an image's allocation base is its module handle; writes `name`.
    let n = unsafe { GetModuleFileNameW(base, name.as_mut_ptr(), name.len() as u32) } as usize;
    let full = String::from_utf16_lossy(&name[..n]);
    full.rsplit(['\\', '/']).next().unwrap_or("an unnamed image").to_string()
}

/// The kinds of memory in `[start, start + len)`, with their bytes.
fn kinds(start: u64, len: u64) -> Vec<(String, u64)> {
    let mut out: BTreeMap<String, u64> = BTreeMap::new();
    let end = start.saturating_add(len);
    let mut at = start;
    while at < end {
        let mut m = MEMORY_BASIC_INFORMATION::default();
        // SAFETY: a query of this process's address space; writes `m`.
        let got = unsafe { VirtualQuery(at as *const _, &mut m, size_of_val(&m)) };
        if got == 0 {
            *out.entry("no answer from VirtualQuery".into()).or_default() += end - at;
            break;
        }
        let region_end = (m.BaseAddress as u64).saturating_add(m.RegionSize as u64).min(end).max(at + 1);
        let bytes = region_end - at;
        let low = if at < 1 << 32 { "below 4 GiB" } else { "above 4 GiB" };
        let kind = if m.State != MEM_COMMIT {
            format!("not committed ({low})")
        } else if m.Type == MEM_IMAGE {
            format!("image of {} ({low})", module_name(m.AllocationBase))
        } else if m.Type == MEM_MAPPED {
            format!("mapped ({low})")
        } else if m.Type == MEM_PRIVATE {
            format!("private ({low})")
        } else {
            format!("type {:#x} ({low})", m.Type)
        };
        *out.entry(kind).or_default() += bytes;
        at = region_end;
    }
    out.into_iter().collect()
}

/// A title buffer object of `len` bytes at `start` gets its first copy:
/// what memory it lies in (said for every one asked about).
pub(crate) fn buffer(start: u64, len: u64) {
    let found = kinds(start, len);
    let what: Vec<String> = found.iter().map(|(k, b)| format!("{k} {:.2} MB", *b as f64 / 1048576.0)).collect();
    tracing::info!(target: "victory_gpu::host", "title buffer {start:#x}+{len:#x}: {}", what.join(", "));
}

/// A new window over `[start, start + len)`: what memory it covers.
pub(crate) fn window(start: u64, len: u64, asked: u64) {
    let found = kinds(start, len);
    // The asked address's own page (the window is cut around it).
    let at_ask = kinds(asked, 1).into_iter().next().map_or_else(String::new, |(k, _)| k);
    let mut totals = TOTALS.lock().unwrap_or_else(|e| e.into_inner());
    // Asks whose own page is not memory: an address that is none.
    if !at_ask.starts_with("mapped") && !at_ask.starts_with("private") {
        totals.entry(format!("ASK NOT IN MEMORY: {at_ask}")).or_default().0 += 1;
    }
    for (kind, bytes) in &found {
        let t = totals.entry(kind.clone()).or_default();
        t.0 += 1;
        t.1 += bytes;
    }
    static SAID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = SAID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    if n < 400 {
        let what: Vec<String> = found.iter().map(|(k, b)| format!("{k} {:.2} MB", *b as f64 / 1048576.0)).collect();
        let all: Vec<String> = totals.iter().map(|(k, (c, b))| format!("{k}: {c} windows {:.0} MB", *b as f64 / 1048576.0)).collect();
        tracing::info!(
            target: "victory_gpu::host",
            "window by address {n}: {start:#x}+{len:#x} for an ask at {asked:#x} (the ask's page: {at_ask}): {}; all windows so far: {}",
            what.join(", "),
            all.join("; ")
        );
    }
}
