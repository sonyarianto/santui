$ErrorActionPreference = 'Stop'

$Dest = "$env:LOCALAPPDATA\santui"
$BinDir = "$Dest\current"

Write-Host ">> Uninstalling santui ..." -ForegroundColor Cyan

# ── remove files ──
if (Test-Path $Dest) {
    Remove-Item $Dest -Recurse -Force
    Write-Host "  Removed $Dest" -ForegroundColor Green
} else {
    Write-Host "  $Dest not found — skipping" -ForegroundColor Yellow
}

# ── remove from PATH ──
$UserPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($UserPath -and $UserPath -like "*$BinDir*") {
    $NewPath = ($UserPath -split ';' | Where-Object { $_ -ne $BinDir }) -join ';'
    [Environment]::SetEnvironmentVariable('PATH', $NewPath, 'User')
    # also update current session
    $env:PATH = ($env:PATH -split ';' | Where-Object { $_ -ne $BinDir }) -join ';'
    Write-Host "  Removed from User PATH" -ForegroundColor Green
} else {
    Write-Host "  Not found in PATH — skipping" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "[OK] Santui has been uninstalled." -ForegroundColor Green
Write-Host "  Restart your terminal to refresh PATH."
