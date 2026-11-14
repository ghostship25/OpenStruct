# Build OpenStruct for Windows and collect the release binaries.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

cargo build --release --locked -p openstruct -p openstruct-gui

$dest = Join-Path $root "builds\windows"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item "target\release\openstruct-gui.exe" (Join-Path $dest "OpenStruct.exe") -Force
Copy-Item "target\release\openstruct.exe" (Join-Path $dest "openstruct-cli.exe") -Force
Copy-Item "target\release\openstruct.dll" (Join-Path $dest "openstruct.dll") -Force

$files = @("OpenStruct.exe", "openstruct-cli.exe", "openstruct.dll")
$lines = foreach ($file in $files) {
    $hash = Get-FileHash -Algorithm SHA256 (Join-Path $dest $file)
    "$($hash.Hash.ToLower())  $file"
}
# LF line endings so `sha256sum -c SHA256SUMS.txt` works on Linux and CI.
[System.IO.File]::WriteAllText(
    (Join-Path $dest "SHA256SUMS.txt"),
    ($lines -join "`n") + "`n",
    [System.Text.Encoding]::ASCII)

Write-Host "Artifacts written to $dest"
