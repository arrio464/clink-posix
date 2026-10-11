# Disable only the loader installed by install.ps1. Runtime files remain available.
$ErrorActionPreference = 'Stop'
$runtime = Join-Path $env:LOCALAPPDATA 'clink-posix-native'
$receiptPath = Join-Path $runtime 'installation.json'
if (-not (Test-Path -LiteralPath $receiptPath -PathType Leaf)) {
    throw 'Managed installation receipt not found; nothing was changed.'
}
$receipt = Get-Content -Raw -LiteralPath $receiptPath | ConvertFrom-Json
$loader = Join-Path $env:USERPROFILE '.clink\00-native.lua'
if ($receipt.loader -ne $loader -or -not (Test-Path -LiteralPath $loader -PathType Leaf)) {
    throw 'Loader path does not match the managed installation; nothing was changed.'
}
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $loader).Hash
if ($hash -ne $receipt.sha256.'00-native.lua') {
    throw 'Loader was edited after installation; nothing was changed.'
}
$disabled = "$loader.disabled"
if (Test-Path -LiteralPath $disabled) {
    throw 'Disabled backup already exists; nothing was changed.'
}
Move-Item -LiteralPath $loader -Destination $disabled -ErrorAction Stop
Write-Output "Disabled native px in new CMD/Clink sessions. Existing Python px loader can load again. Backup: $disabled"
