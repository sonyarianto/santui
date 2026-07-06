$ErrorActionPreference = 'Stop'

$Repo = 'sonyarianto/santui'
$Dest = "$env:LOCALAPPDATA\santui"
$BinDir = "$Dest\current"

# ── detect arch ──
$Arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64'  { 'x86_64-pc-windows-msvc' }
    default { throw "Unsupported architecture: $env:PROCESSOR_ARCHITECTURE (only x86_64 is available)" }
}

Write-Host ">> Fetching latest release ..." -ForegroundColor Cyan
$ApiUrl = "https://api.github.com/repos/$Repo/releases/latest"
try {
    $Release = Invoke-RestMethod -Uri $ApiUrl -UseBasicParsing
} catch {
    Write-Host "  [!] No release found on GitHub yet. Build from source instead:" -ForegroundColor Yellow
    Write-Host "  git clone https://github.com/$Repo.git" -ForegroundColor Cyan
    Write-Host "  cd santui && cargo build --workspace && cargo run -p santui"
    exit 1
}
$Tag = $Release.tag_name
$ZipUrl = "https://github.com/$Repo/releases/download/$Tag/santui-$Arch.zip"

Write-Host ">> Installing santui ($Arch)..." -ForegroundColor Cyan

# ── download ──
$Tmp = Join-Path ([System.IO.Path]::GetTempPath()) "santui-$([System.IO.Path]::GetRandomFileName()).zip"
Write-Host "  Downloading $ZipUrl ..."
Invoke-WebRequest -Uri $ZipUrl -OutFile $Tmp -UseBasicParsing

# ── unblock ZIP first so MOTW doesn't propagate to extracted files ──
Write-Host "  Unblocking downloaded archive ..."
Unblock-File -Path $Tmp

# ── extract ──
Write-Host "  Extracting ..."
if (Test-Path $BinDir) { Remove-Item $BinDir -Recurse -Force }
New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
Expand-Archive -Path $Tmp -DestinationPath $BinDir -Force
Remove-Item $Tmp -Force

# ── force unblock: remove Zone.Identifier stream directly ──
Write-Host "  Unblocking installed binary ..."
$exe = Join-Path $BinDir "santui.exe"
Remove-Item -Path $exe -Stream Zone.Identifier -ErrorAction SilentlyContinue

# Also remove inherited MOTW from any sibling files
Get-ChildItem -LiteralPath $BinDir -Recurse -File | ForEach-Object {
    Remove-Item -Path $_.FullName -Stream Zone.Identifier -ErrorAction SilentlyContinue
}

# ── PATH ──
$UserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($UserPath -notlike "*$BinDir*") {
    Write-Host "  Adding to PATH ..."
    $NewPath = if ($UserPath) { "$UserPath;$BinDir" } else { $BinDir }
    [Environment]::SetEnvironmentVariable('PATH', $NewPath, 'User')
    # also update current session
    $env:PATH = "$env:PATH;$BinDir"
}

# ── verify file exists and has reasonable size ──
if (Test-Path $exe) {
    $size = (Get-Item $exe).Length
    if ($size -gt 1mb) {
        Write-Host "[OK] santui v0.2.28 ($([math]::Round($size / 1mb, 1)) MB)" -ForegroundColor Green
    } else {
        Write-Host "  [!] santui.exe seems too small ($size bytes)" -ForegroundColor Yellow
    }
}

# ── Windows Defender exclusion ──
Write-Host "  Configuring Windows Defender exclusion ..." -ForegroundColor Cyan
$IsAdmin = [Security.Principal.WindowsPrincipal]::new(
    [Security.Principal.WindowsIdentity]::GetCurrent()
).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if ($IsAdmin) {
    try {
        Add-MpPreference -ExclusionPath $BinDir -ErrorAction Stop | Out-Null
        Write-Host "    Added exclusion: $BinDir" -ForegroundColor Green
    } catch {
        Write-Host "    [!] Could not add Defender exclusion: $($_.Exception.Message)" -ForegroundColor Yellow
        Write-Host "    Run as Administrator next time for auto-exclusion."
    }
} else {
    Write-Host "    Skipped (not running as Administrator)." -ForegroundColor Yellow
    Write-Host "    If santui is blocked, run this as Admin:" -ForegroundColor Yellow
    Write-Host "    Add-MpPreference -ExclusionPath \"$BinDir\"" -ForegroundColor Cyan
}

Write-Host "  Run santui from any terminal."
