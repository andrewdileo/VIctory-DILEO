# Changelog

All notable changes to VIctory-DILEO are listed here.
Maintained by Andrew DiLeo ([@andrewdileo](https://github.com/andrewdileo)).

## [0.1.1] - 2026-11-26

- Restored the shader translator and cache identity used before the recent shader integrations. Kept the raw buffer view bounds fix and the Series S FSR scale guard.
- Added checksum-verified, portable Windows packages with separate saves, caches, and logs. [Windows release guide](docs/releases.md)

## [0.2.0-rc.1] - 2026-11-26

- Published the shader experiment as a separate prerelease. This version creates the alpha transmission pipeline on an RTX 5090. It is retained for comparison because gameplay showed shader and NPC face-texture regressions.

## [0.1.0] - 2026-11-26

First public release, built from the internal RTX 5090 aggregate build.

### Added
- RTX 5090 launcher `Play-5090.cmd` (`scripts/play-aggregate.ps1`) with the
  NVIDIA Series S preset, per-run log folders and `-FSR`, `-RenderScale`,
  `-StatsOverlay`, `-Quality`, `-RayTracing`, `-Checkpoint`, `-NoWatchdog`
  and `-Extra` switches.
- F3 diagnostics sidebar showing the stats that matter for the next tuning
  step.
- Displayed-frame statistics, isolated checkpoint runs (hash-checked save
  copies) and a timing driver.
- NVIDIA Nsight Aftermath GPU crash diagnostics, kept separate from gameplay
  crash collection. PIX capture attach.
- Frame capture at full size with `VICTORY_CAPTURE_FULL=1` (half size
  otherwise), plus replay inspection.
- Card selection on multi-GPU PCs (`VICTORY_ADAPTER`).
- Per-console, per-resolution launchers (`scripts/play-S-*.ps1`,
  `scripts/play-X-*.ps1`).

### Changed
- Dynamic descriptor slots can take up to half of NVIDIA's capped
  shader-visible heap, so draws are no longer dropped. Refused tables are
  counted by cause.
- Series S interface plane composed over the game image (`VICTORY_COMPOSE`).
- Faces animate: 16-bit integer data is kept as integers.
- Ray-tracing resolve and traced-shadow passes skipped by default on NVIDIA.
- Picture fixes: NaN scrubbing, bounded ambient and fog light, intro video
  flicker fix, each pipeline waited for at most once.
- Submission speedups: threaded submission, write relay, card-side fills and
  compares, texture copy queue, heap records compared in place, frame meter
  on its own thread.
- Pooled textures at 64 KiB, and a slot isn't switched to a pooled texture
  until earlier command lists that may read it have run.
- Upload-queue fence fix and a narrow-wave barrier guard.

### Known issues
- Visual faults remain (skin textures on some characters, bloom hot spots,
  tearing outdoors). See `docs/findings.md`.
- Some pipelines are still refused by the driver ("The parameter is
  incorrect"), and a pipeline creation method is not implemented yet.

### Local result
- One RTX 5090 PC, Series S profile, FSR from 720p to 1080p: about 23 fps
  (43.5 ms per frame) over a 70-second session with no crashes or
  device-removed errors.
