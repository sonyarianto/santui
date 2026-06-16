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

# ── extract ──
Write-Host "  Extracting ..."
if (Test-Path $BinDir) { Remove-Item $BinDir -Recurse -Force }
New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
Expand-Archive -Path $Tmp -DestinationPath $BinDir -Force
Remove-Item $Tmp -Force

# ── PATH ──
$UserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($UserPath -notlike "*$BinDir*") {
    Write-Host "  Adding to PATH ..."
    $NewPath = if ($UserPath) { "$UserPath;$BinDir" } else { $BinDir }
    [Environment]::SetEnvironmentVariable('PATH', $NewPath, 'User')
    # also update current session
    $env:PATH = "$env:PATH;$BinDir"
}

Write-Host "[OK] Installed to $BinDir" -ForegroundColor Green
Write-Host "  Run santui from any terminal."
