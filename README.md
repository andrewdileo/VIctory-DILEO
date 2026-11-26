# VIctory-DILEO

A build of **VIctory**, a compatibility layer that runs an Xbox Series title
natively on Windows from files you supply, tuned for the **NVIDIA GeForce
RTX 5090** (and NVIDIA cards in general).

Maintained by **Andrew DiLeo ([@andrewdileo](https://github.com/andrewdileo))**.

To our knowledge, this is the furthest-along public build of the VIctory
compatibility layer.

> **No game files are included.** This repository contains no game code,
> executables, assets or saves. You need your own legally obtained copy of the
> game. This project is not affiliated with or endorsed by the game's developer
> or publisher, or by Microsoft or NVIDIA.

## What's in this build

This tree brings together NVIDIA-focused work on top of VIctory and adds RTX
5090 diagnostics and launch tooling:

- **Draws everything on NVIDIA.** NVIDIA caps the shader-visible descriptor
  heap at 1,000,000 views, below what the title asks for. The pool of dynamic
  descriptor slots can now take up to half the heap (`VICTORY_DYNAMIC_SLOTS`),
  so draws are no longer dropped. Refused tables are counted by cause.
- **Series S interface plane** (`VICTORY_COMPOSE=1`): menus, loading screens
  and subtitles are composed over the game image instead of staying black.
- **Series S profile at a reduced render scale** with the title's own upscaler
  or FSR (`-FSR`), plus title patches for render scale and the watchdog.
- **Faces move**: 16-bit integer data in the face network is no longer read as
  floats.
- **Ray tracing mostly off by default** (`scripts/rt_off_hashes.txt`): the
  ray-tracing structure builder can't run on NVIDIA, so the passes that turned
  misses into overbright rooms are skipped. `-RayTracing` keeps them.
- **Picture fixes**: NaN scrubbing (no black blobs), bounded ambient and fog
  light, the intro video no longer flickers, each new pipeline is waited for at
  most once.
- **Submission and copy-path speedups**: threaded submission, write relay,
  card-side fills and compares, a separate texture copy queue, pooled textures
  at 64 KiB, heap records compared in place.
- **Diagnostics**: an F3 stats sidebar (`-StatsOverlay`), displayed-frame
  statistics, NVIDIA Nsight Aftermath crash dumps (`-CrashDumps`,
  `-GpuDiagnostics`), PIX attach (`-Pix`), frame capture and replay
  ([docs/replay.md](docs/replay.md)), and isolated checkpoint runs.

See [CHANGELOG.md](CHANGELOG.md) for release notes,
[docs/architecture.md](docs/architecture.md) for how the layer is put together,
and [docs/findings.md](docs/findings.md) for measurements and known issues.

## Tested hardware

Developed and tested on one Windows PC:

| Component | Details |
|---|---|
| GPU | NVIDIA GeForce RTX 5090, 32 GB |
| NVIDIA driver | 616.92 |
| CPU | AMD Ryzen 9 9950X (16 cores, 32 threads) |
| RAM | 64 GB |
| OS | Windows 11 Pro, version 26H2 (build 26300) |

## Requirements

- Windows 11, 64-bit
- An NVIDIA GeForce RTX card (developed and tested on an RTX 5090)
- [Rust](https://rustup.rs/) (the toolchain pinned in `rust-toolchain.toml` is
  installed automatically by rustup)
- Your own copy of the game

## Build

```powershell
cargo build --locked --profile diag -p victory --features diagnostics
```

This produces `target\diag\victory.exe`, the diagnostics build every launcher
and measurement here uses. A plain `cargo build --release -p victory` also works,
but it ignores the ray-tracing skip list (that's a diagnostics switch).

## Run

From the repository root, after building:

```powershell
.\Play-5090.cmd -GameExe "<path to your copy's executable>" -FSR -StatsOverlay
```

`Play-5090.cmd` calls `scripts\play-aggregate.ps1`, which runs the Series S
profile with the NVIDIA preset. Each run gets its own folder with a
`game.log` under `%LOCALAPPDATA%\VIctory-aggregate-5090`. Run one session at a
time.

Useful switches:

| Switch | What it does |
|---|---|
| `-FSR` | Uses the Series S profile's own upscaler choice (FSR) at the title's own render scale, instead of forcing the title's temporal upscaler. |
| `-RenderScale 0.4\|0.5\|0.75\|default` | The title's internal render scale (the preset pins 0.5). `default` applies no patch. Not combined with `-FSR`. |
| `-StatsOverlay` | Shows the diagnostics sidebar at startup. F3 toggles it. |
| `-Quality` | Fewer stretched bodies and less rainbow skin, at a lower frame rate. |
| `-RayTracing` | Keeps the ray-tracing passes the skip list leaves out. |
| `-CrashDumps` / `-GpuDiagnostics` | NVIDIA Nsight Aftermath dumps, plus the D3D12 debug layer for the latter. |
| `-SaveSource <folder>` | A save folder copied in on first launch. |
| `-Checkpoint <folder>` | Runs from a hash-checked copy of a save folder, so the run never touches the original. |
| `-NoWatchdog` | Disarms the title's watchdog on very slow frames. |
| `-Extra @{VICTORY_...='...'}` | Any other `VICTORY_*` setting. |

Per-console and per-resolution launchers are also in `scripts\`
(`play-S-720p.ps1`, `play-X-1080p.ps1` and others), all built on
`scripts\play-nvidia.ps1 -Exe <path>`.

Before the first run you can translate shaders and warm pipelines ahead of time:

```powershell
target\diag\victory.exe shaders <the shader archives>
target\diag\victory.exe warm --used
```

## Where it stands

Local result on one RTX 5090 PC: launched with
`Play-5090.cmd -FSR -StatsOverlay` (Series S profile, FSR from 720p to
1080p), the game ran steadily for a 70-second session at about **23 fps
(43.5 ms per frame)** with no crashes or device-removed errors. Some
pipeline-creation warnings remain in the log. Expect visual faults: see
[docs/findings.md](docs/findings.md).

## Asset policy

Nothing from the game is ever stored in this repository. See
[docs/asset-policy.md](docs/asset-policy.md). The one exception is the BCPack
format's constant tables (`crates/victory-formats/src/bcpack-tables.bin`),
which are part of the format and not covered by this repository's license.

## License and credits

Licensed under the **GNU General Public License v3.0 or later**. See
[LICENSE](LICENSE).

This project is derived from the VIctory project and the work of its community
contributors, and is distributed under the same GPL-3.0-or-later license.
Existing copyright and license notices in the source (including third-party
code under MIT and GPL-2.0 terms) are kept as required.

NVIDIA / RTX 5090 tuning, packaging and maintenance of this build: Andrew
DiLeo ([@andrewdileo](https://github.com/andrewdileo)).
