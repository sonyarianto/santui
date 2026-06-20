$ErrorActionPreference = 'Stop'

$Root = Split-Path $PSScriptRoot -Parent
$OutDir = "$Root\target\debug"

Write-Host ">> Building workspace (debug) ..." -ForegroundColor Cyan
cargo build --workspace
if ($LASTEXITCODE -ne 0) { throw "build failed" }

# -- copy native assets --
Write-Host ">> Copying native assets to $OutDir\native\ ..."
if (Test-Path "$Root\native") {
    New-Item -ItemType Directory -Path "$OutDir\native" -Force | Out-Null
    Get-ChildItem -LiteralPath "$Root\native" | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination "$OutDir\native\" -Force
        Write-Host "  [OK] $($_.Name)"
    }
}

# -- generate plugins.json --
Write-Host ">> Generating plugins.json ..."
$pluginBinaries = Get-ChildItem -LiteralPath $OutDir -Filter "santui-*.exe" | Where-Object {
    $_.Name -notmatch 'scraper'
}

$plugins = @()
foreach ($bin in $pluginBinaries) {
    $id = $bin.BaseName -replace '^santui-', ''
    $hash = (Get-FileHash -LiteralPath $bin.FullName -Algorithm SHA256).Hash
    Write-Host "  [OK] $id  ($($bin.Length) bytes, sha256=$hash)"
    $plugins += @{
        id            = $id
        name          = "Radio Streaming Player"
        description   = "Listen to 50,000+ radio stations"
        version       = "0.1.9"
        download_url  = "target/debug/$($bin.Name)"
        sha256        = $hash
        size          = $bin.Length
    }
}

$json = $plugins | ConvertTo-Json -Compress
if ($json -notmatch '^\[.*\]$') { $json = "[$json]" }
[System.IO.File]::WriteAllText("$Root\plugins.json", $json, [System.Text.UTF8Encoding]::new($false))
Write-Host ("[OK] plugins.json generated ({0} plugin{1})" -f $plugins.Count, $(if ($plugins.Count -ne 1) { 's' } else { '' })) -ForegroundColor Green
