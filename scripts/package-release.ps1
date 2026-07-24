$ErrorActionPreference = 'Stop'

$Root = Split-Path $PSScriptRoot -Parent
$Target = "$Root\target\release"
$OutDir = "$Root\releases"

$Version = if ($args[0]) { $args[0] } else { "nightly" }

$Arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64'  { 'x86_64-pc-windows-msvc' }
    'ARM64'  { 'aarch64-pc-windows-msvc' }
    default { throw "Unsupported arch: $env:PROCESSOR_ARCHITECTURE" }
}

# ── build ──
Write-Host "» Building release binaries ..." -ForegroundColor Cyan
cargo build --release --workspace
if ($LASTEXITCODE -ne 0) { throw "build failed" }

# ── stage ──
$Stage = Join-Path ([System.IO.Path]::GetTempPath()) "santui-pkg\$Version"
if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
New-Item -ItemType Directory -Path "$Stage\native" -Force | Out-Null

Copy-Item "$Target\santui.exe" $Stage
Copy-Item "$Root\native\libmpv-2.dll" "$Stage\native\"
Copy-Item "$Root\native\radio_stream_stations.db" "$Stage\native\"

# Copy all plugin binaries (santui-*) into the archive
Get-ChildItem "$Target\santui-*" -File | ForEach-Object {
    Copy-Item $_.FullName $Stage
}

# ── zip ──
$ZipName = "santui-$Arch.zip"
$ZipPath = "$OutDir\$ZipName"
if (Test-Path $ZipPath) { Remove-Item $ZipPath -Force }

Write-Host "  Packing $ZipName ..."
Add-Type -Assembly System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($Stage, $ZipPath)

# ── clean stage ──
if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }

# ── done ──
$Size = "{0:N0} KB" -f ((Get-Item $ZipPath).Length / 1KB)
Write-Host "✔ $ZipName ($Size)" -ForegroundColor Green
