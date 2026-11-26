# VIctory-DILEO Windows releases

Each GitHub release has an immutable version tag and a Windows x64 archive with a SHA-256 checksum. `build-info.json` records the exact source commit and executable checksum. The release contains the compatibility layer. Supply your own game files and keep them outside the source checkout. [Source and releases](https://github.com/andrewdileo/VIctory-DILEO/releases)

## Run a packaged version

Extract each archive into its own folder named for the tag. In PowerShell, open that folder and use the path to your game executable.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\play-aggregate.ps1 `
  -GameExe 'D:\Games\MyCopy\game_scarlett_master.exe' -FSR -TextureWait -StatsOverlay
```

Each package defaults to its own `runtime` folder for saves, shader and pipeline caches, and run logs. Use `-SaveSource` on the first launch to copy an existing data folder into that version. Use `-Checkpoint` to copy a save into one disposable run. `-RunRoot` selects a different runtime folder. Source checkouts use the existing local runtime path unless a run root is supplied.

Verify the archive before extraction with `Get-FileHash -Algorithm SHA256` and compare its hash to the published `.sha256` file.

## Keep versions side by side

Use `main` for the current source and a separate branch or worktree for experiments. Tag a revision after checking that its workspace version and `Cargo.lock` agree. Make a patch release for a rollback or small fix. Use a prerelease suffix for experimental changes that still need gameplay validation. Publish a new tag when a released revision changes.

To build a tagged version locally, check it out in a clean worktree and run `scripts/package-windows.ps1`. It builds with the pinned Rust toolchain and `Cargo.lock`, then places the expanded build, ZIP and checksum in the sibling `VIctory-DILEO-builds` folder. Pass `-Offline` if the toolchain and dependencies are already cached.
