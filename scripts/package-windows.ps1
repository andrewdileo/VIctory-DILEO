[CmdletBinding()]
param(
    [string]$OutputDirectory,
    [switch]$Offline,
    [ValidateRange(1, 128)][int]$Jobs = 4
)

$ErrorActionPreference = 'Stop'
$source = Split-Path -Parent $PSScriptRoot
if (!$OutputDirectory) {
    $OutputDirectory = Join-Path (Split-Path -Parent $source) 'VIctory-DILEO-builds'
}
$output = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if ($output.Equals($source, [StringComparison]::OrdinalIgnoreCase) -or
    $output.StartsWith($source + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Use a build directory outside the source checkout.'
}

Push-Location -LiteralPath $source
try {
    $dirty = git status --porcelain
    if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'Packaging requires a clean Git checkout.' }
    $commit = git rev-parse HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the source commit.' }

    $cargoMode = @()
    if ($Offline) { $cargoMode += '--offline' }
    $metadataJson = cargo metadata --locked --no-deps --format-version 1 @cargoMode
    if ($LASTEXITCODE -ne 0) { throw 'Could not read the locked workspace version.' }
    $metadata = $metadataJson | ConvertFrom-Json
    $version = ($metadata.packages | Where-Object name -eq 'victory').version
    if ($version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?$') {
        throw 'Expected a semantic version for the victory package.'
    }
    $tag = "v$version"
    $tagCommit = git rev-parse --verify "$tag^{commit}"
    if ($LASTEXITCODE -ne 0 -or $tagCommit -ne $commit) { throw "Check out tag $tag before packaging." }

    $name = "VIctory-DILEO-$tag-windows-x64"
    $folder = Join-Path $output $tag
    $archive = Join-Path $output "$name.zip"
    $checksums = Join-Path $output "$name.sha256"
    foreach ($path in $folder, $archive, $checksums) {
        if (Test-Path -LiteralPath $path) { throw "Release output already exists at $path." }
    }
    New-Item -ItemType Directory -Force -Path $output | Out-Null
    $output = (Resolve-Path -LiteralPath $output).Path
    $target = Join-Path $output '.cargo-target'
    $stage = Join-Path $output ('.stage-' + [guid]::NewGuid())

    $savedFlags = $env:CARGO_ENCODED_RUSTFLAGS
    $savedDebug = $env:CARGO_PROFILE_DIAG_DEBUG
    try {
        $remaps = @("--remap-path-prefix=$source=.")
        if ($env:USERPROFILE) { $remaps += "--remap-path-prefix=$env:USERPROFILE=/local" }
        $env:CARGO_ENCODED_RUSTFLAGS = $remaps -join [char]31
        $env:CARGO_PROFILE_DIAG_DEBUG = '0'
        cargo build --locked @cargoMode -j $Jobs --target x86_64-pc-windows-msvc --target-dir $target --profile diag -p victory --features diagnostics
        if ($LASTEXITCODE -ne 0) { throw 'The diagnostics build failed.' }
    } finally {
        $env:CARGO_ENCODED_RUSTFLAGS = $savedFlags
        $env:CARGO_PROFILE_DIAG_DEBUG = $savedDebug
    }

    $exeDir = Join-Path $stage 'target\diag'
    $scriptsDir = Join-Path $stage 'scripts'
    New-Item -ItemType Directory -Force -Path $exeDir, $scriptsDir | Out-Null
    $exe = Join-Path $exeDir 'victory.exe'
    Copy-Item -LiteralPath (Join-Path $target 'x86_64-pc-windows-msvc\diag\victory.exe') -Destination $exe
    foreach ($script in 'play-aggregate.ps1', 'play-nvidia.ps1', 'rt_off_hashes.txt') {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $script) -Destination $scriptsDir
    }
    Copy-Item -LiteralPath (Join-Path $source 'LICENSE') -Destination $stage
    Copy-Item -LiteralPath (Join-Path $source 'docs\releases.md') -Destination (Join-Path $stage 'README.md')
    $about = & $exe --version
    if ($LASTEXITCODE -ne 0 -or $about -ne "victory $version") { throw 'The packaged executable reports the wrong version.' }
    [ordered]@{
        version = $version
        tag = $tag
        commit = $commit
        source = "https://github.com/andrewdileo/VIctory-DILEO/tree/$tag"
        target = 'x86_64-pc-windows-msvc'
        profile = 'diag'
        executableSha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $stage 'build-info.json') -Encoding UTF8

    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -CompressionLevel Optimal
    $zipHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$zipHash  $name.zip" | Set-Content -LiteralPath $checksums -Encoding ASCII
    [IO.Directory]::Move($stage, $folder)
    Write-Output $folder, $archive, $checksums
} finally {
    Pop-Location
}
