[CmdletBinding()]
param(
    # Your own copy's executable (no game files are included in this repository).
    [Parameter(Mandatory)][string]$GameExe,
    # Optional: a save folder copied in on first launch. Without it the run starts with an empty data folder.
    [string]$SaveSource,
    [switch]$GpuDiagnostics,
    [switch]$CrashDumps,
    [switch]$CaptureWorld,
    # Show the live diagnostics sidebar on startup. F3 toggles it.
    [switch]$StatsOverlay,
    # A/B: allow wave-wide translation only for the reflection tile compute.
    [switch]$PrisonReflectionExperiment,
    [switch]$IsolateDensityShadowMips,
    # Load the PIX GPU capturer so PIX can attach and capture a frame.
    [switch]$Pix,
    # A/B: wait for title memory to hold still before reading a texture
    # (the NVIDIA preset reads without waiting).
    [switch]$TextureWait,
    # A/B: the title's own ambient light instead of the stand-in estimate.
    [switch]$NoAmbientEstimate,
    # A/B: the title's internal render scale (the NVIDIA preset pins 0.5).
    # 1.0 stops the title at start (its own check, 0x80000003 at +0x2219650,
    # run 20261125-222830-0273); FSR has constants for 0.5, 0.4, 1/3, 0.75.
    # 'default': no scale patch, the title's own (2/3 on Series S).
    [ValidateSet('0.4', '0.5', '0.75', 'default')]
    [string]$RenderScale,
    # A/B: the Series S profile's own upscaler choice instead of the title's
    # temporal upscaler forced on (the NVIDIA preset forces it). Series S
    # picks FSR, which needs its own 1280x720 render size: with a pinned
    # scale it divides by zero (+0x28f03e8, run 20261125-222913-1463), so this
    # implies -RenderScale default.
    [Alias('FSR')]
    [switch]$NoOwnUpscaler,
    # Passed to play-nvidia.ps1: -Quality waits where
    # the speed set does not (relay 1, no card steps, textures wait; fewer
    # stretched bodies, less rainbow skin, slower). -RayTracing keeps the
    # 1,034 ray-tracing shaders its skip list leaves out.
    [switch]$Quality,
    [switch]$RayTracing,
    # Saves the presented picture (half size, frame-<n>.png in the run's
    # folder) every this many seconds; each saved frame is read back at
    # once and takes longer than its neighbours. 0: off.
    [ValidateRange(0, 3600)]
    [int]$ScreenshotEvery = 0,
    # The title's watchdog ends the process with a deliberate write to 0
    # (+0x23cf683) when its main thread stalls: run 20261125-225025-5576
    # after a 6.4 s frame while loading into the world. This disarms it
    # (victory-loader hooks.rs, checked against the 1.0.2.1 bytes).
    [switch]$NoWatchdog,
    # Any other VICTORY_* settings, last (they win): -Extra @{VICTORY_WINDOWS_BAR_MB='2048'}
    [hashtable]$Extra = @{},
    # A save folder (like ...\data) copied into this run's own folder and used
    # from there: the run's saves never touch the shared one or the source.
    # Every file is checked by SHA-256 after the copy.
    [string]$Checkpoint,
    # Override where this installation keeps saves, caches, and run logs.
    [string]$RunRoot
)
$ErrorActionPreference = 'Stop'
$game = Get-Item -LiteralPath $GameExe
if (!(Test-Path -LiteralPath "$PSScriptRoot\..\target\diag\victory.exe")) { throw 'Build the diagnostics binary first.' }
$installRoot = Split-Path -Parent $PSScriptRoot
if ($RunRoot) {
    $root = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($RunRoot)
} elseif (Test-Path -LiteralPath (Join-Path $installRoot 'build-info.json') -PathType Leaf) {
    $root = Join-Path $installRoot 'runtime'
} else {
    $root = Join-Path $env:LOCALAPPDATA 'VIctory-aggregate-5090'
}
$data = Join-Path $root 'data'
New-Item -ItemType Directory -Force -Path $root | Out-Null
if (!(Test-Path -LiteralPath $data)) {
    if ($SaveSource) {
        $seed = Get-Item -LiteralPath $SaveSource
        $temp = Join-Path $root ([guid]::NewGuid().ToString())
        Copy-Item -LiteralPath $seed.FullName -Destination $temp -Recurse
        [IO.Directory]::Move($temp, $data)
    } else {
        New-Item -ItemType Directory -Path $data | Out-Null
    }
}
$run = Join-Path $root (Get-Date -Format 'yyyyMMdd-HHmmss-ffff')
New-Item -ItemType Directory -Path $run | Out-Null
if ($Checkpoint) {
    $from = (Get-Item -LiteralPath $Checkpoint).FullName
    $data = Join-Path $run 'data'
    Copy-Item -LiteralPath $from -Destination $data -Recurse
    $files = @(Get-ChildItem -LiteralPath $from -Recurse -File)
    # (.NET's SHA-256: Get-FileHash is not there in every PowerShell 5 session.)
    $sha = [Security.Cryptography.SHA256]::Create()
    $hash = { param($p) $s = [IO.File]::OpenRead($p); try { [BitConverter]::ToString($sha.ComputeHash($s)) } finally { $s.Dispose() } }
    foreach ($f in $files) {
        $copy = Join-Path $data $f.FullName.Substring($from.Length).TrimStart('')
        if ((& $hash $f.FullName) -ne (& $hash $copy)) { throw "Checkpoint copy differs: $copy" }
    }
    Write-Host "Checkpoint: $($files.Count) files from $from, hash-checked, in $data"
}
$env:VICTORY_DATA_DIR = $data
$env:VICTORY_CACHE_DIR = Join-Path $root 'cache'
$env:VICTORY_GAME_DIR = $game.DirectoryName
$opts = @{ VICTORY_LOG = 'warn,victory_gpu::host=info,victory_gpu::copystat=info,victory_hle::modules::lifecycle=info' }
if ($StatsOverlay) {
    $opts.VICTORY_PANEL = '1'
}
if ($PrisonReflectionExperiment) {
    $opts.VICTORY_PRISON_REFLECTION_EXPERIMENT = '1'
}
if ($GpuDiagnostics -or $CrashDumps) {
    $opts.VICTORY_AFTERMATH = '1'
    $opts.VICTORY_AFTERMATH_DIR = $run
    $opts.VICTORY_LOG += ',victory_gpu::aftermath=info'
}
if ($GpuDiagnostics) {
    $opts.VICTORY_AFTERMATH_SHADER_ERRORS = '1'
    $opts.VICTORY_D3D12_DEBUG = '1'
    $opts.VICTORY_D3D12_DRED = 'light'
    $opts.VICTORY_LOG += ',victory_gpu::capture=info,victory_gpu::draw=info'
}
if ($CaptureWorld) {
    $opts.VICTORY_CAPTURE = $run
    $opts.VICTORY_CAPTURE_WORLD = '1'
}
if ($NoWatchdog) {
    $opts.VICTORY_NO_WATCHDOG = '1'
    Write-Host "The title's watchdog is disarmed."
}
if ($ScreenshotEvery -gt 0) {
    $opts.VICTORY_CAPTURE = $run
    $opts.VICTORY_CAPTURE_EVERY_S = "$ScreenshotEvery"
    Write-Host "Screenshots every $ScreenshotEvery s into $run (frame-<n>.png)."
}
if ($IsolateDensityShadowMips) {
    # RTX 5090 Aftermath dump from the November 25 loading crash. Added to
    # play-nvidia.ps1's ray-tracing skip list, not in place of it (-More
    # replaces a setting).
    $skip = @('5b2ce592c3a5dd09')
    $rtList = Join-Path $PSScriptRoot 'rt_off_hashes.txt'
    if (-not $RayTracing -and (Test-Path $rtList)) { $skip = @((Get-Content $rtList -Raw).Trim()) + $skip }
    $opts.VICTORY_SKIP_PIPELINES = $skip -join ','
    Write-Host 'Diagnostic comparison: skipping CS_DensityShadowMapMips (5b2ce592c3a5dd09).'
}
if ($Pix) {
    if ($GpuDiagnostics -or $CrashDumps) { throw 'Use -Pix without -GpuDiagnostics or -CrashDumps: Aftermath and the debug layer interfere with PIX captures.' }
    $opts.VICTORY_PIX = '1'
    $opts.VICTORY_LOG += ',victory_gpu::pix=info'
    Write-Host 'PIX capturer on: in PIX, Attach to victory.exe, then take a GPU capture at the scene.'
}
if ($TextureWait) {
    $opts.VICTORY_TEXTURE_NO_WAIT = '0'
    Write-Host 'A/B: textures wait for title memory to hold still.'
}
if ($NoAmbientEstimate) {
    $opts.VICTORY_AMBIENT = '0'
    Write-Host "A/B: the title's own ambient light (stand-in estimate off)."
}
if ($NoOwnUpscaler) {
    if ($PSBoundParameters.ContainsKey('RenderScale') -and $RenderScale -ne 'default') {
        throw '-NoOwnUpscaler needs the title''s own scale: FSR stops on a pinned one. Leave out -RenderScale.'
    }
    $RenderScale = 'default'
    $opts.VICTORY_OWN_UPSCALER = '0'
    Write-Host "A/B: the title's own upscaler is not forced (Series S picks FSR)."
}
if ($RenderScale) {
    # 'off' is no number: the loader warns once and patches no scale.
    $opts.VICTORY_RENDER_SCALE = if ($RenderScale -eq 'default') { 'off' } else { $RenderScale }
    Write-Host "A/B: render scale $RenderScale."
}
foreach ($k in $Extra.Keys) {
    if ($k -notlike 'VICTORY_*') { throw "-Extra takes VICTORY_* settings only, not $k" }
    $opts[$k] = [string]$Extra[$k]
    Write-Host "Extra: $k=$($Extra[$k])"
}
Write-Host "Run log in $run"
# Windows PowerShell 5 can promote redirected native stderr to a stopping error.
$ErrorActionPreference = 'Continue'
$nvidia = @{ Exe = $game.FullName; NoSmoothMotion = $true; More = $opts }
if ($Quality) { $nvidia.Quality = $true; Write-Host 'Quality set (play-nvidia.ps1 -Quality).' }
if ($RayTracing) { $nvidia.RayTracing = $true; Write-Host 'Ray-tracing passes kept (play-nvidia.ps1 -RayTracing).' }
& "$PSScriptRoot\play-nvidia.ps1" @nvidia *> (Join-Path $run 'game.log')
exit $LASTEXITCODE
