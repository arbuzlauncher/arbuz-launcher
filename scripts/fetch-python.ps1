# Downloads a portable Python runtime for the embedded plugin engine (PyO3).
# Run once before building the Windows installer. The runtime is NOT committed.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/fetch-python.ps1
#
# Result: src-tauri/python-runtime/ with python3.dll, python312.dll and
# python312.zip (stdlib). The installer copies them next to arbuz-launcher.exe.
$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root 'src-tauri\python-runtime'
$ver = '3.12.10'
$zip = Join-Path $env:TEMP "python-$ver-embed-amd64.zip"
$url = "https://www.python.org/ftp/python/$ver/python-$ver-embed-amd64.zip"

New-Item -ItemType Directory -Force $out | Out-Null
if (-not (Test-Path $zip)) {
  Write-Host "downloading $url"
  Invoke-WebRequest -Uri $url -OutFile $zip -Headers @{ 'User-Agent' = 'arbuz-launcher' }
}
Expand-Archive -Path $zip -DestinationPath $out -Force

# The embeddable package may not include python3.dll (stable ABI shim).
if (-not (Test-Path (Join-Path $out 'python3.dll'))) {
  $nuget = Join-Path $env:TEMP "python-$ver.nupkg"
  if (-not (Test-Path $nuget)) {
    Write-Host "downloading python3.dll from NuGet"
    Invoke-WebRequest -Uri "https://www.nuget.org/api/v2/package/python/$ver" -OutFile $nuget -Headers @{ 'User-Agent' = 'arbuz-launcher' }
  }
  $tmp = Join-Path $env:TEMP "python-$ver-nuget"
  if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
  Expand-Archive -Path $nuget -DestinationPath $tmp -Force
  Copy-Item (Join-Path $tmp 'tools\python3.dll') $out -Force
}

Get-ChildItem $out | Select-Object Name, Length
Write-Host "python runtime: $out"
