# Install or upgrade a managed native px adapter; preserve the Python files.
param([switch]$Upgrade)
$ErrorActionPreference = 'Stop'
$runtime = Join-Path $env:LOCALAPPDATA 'clink-posix-native'
$loader = Join-Path $env:USERPROFILE '.clink\00-native.lua'
$sourceExe = Join-Path $PSScriptRoot 'clink-posix-native.exe'
$sourceAdapter = Join-Path $PSScriptRoot 'native-preview.lua'
$sourceManifest = Join-Path $PSScriptRoot 'tools.tsv'
if (-not (Test-Path -LiteralPath $sourceManifest)) {
    $sourceManifest = Join-Path $PSScriptRoot 'tools.example.tsv'
}
foreach ($path in @($sourceExe, $sourceAdapter, $sourceManifest)) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Build or source file is missing: $path"
    }
}
if ($Upgrade) {
    & (Join-Path $PSScriptRoot 'upgrade.ps1')
    return
}
if ((Test-Path -LiteralPath $runtime) -or (Test-Path -LiteralPath $loader)) {
    throw 'Managed runtime or 00-native.lua already exists; nothing was changed.'
}
$forward = $runtime.Replace('\', '/')
if ($forward -match '[%!"\r\n]') {
    throw 'Runtime path contains a character unsafe for the preview CMD transport.'
}
$clinkHome = Split-Path -Parent $loader
if (-not (Test-Path -LiteralPath $clinkHome -PathType Container)) {
    throw "Existing Clink script directory is missing: $clinkHome"
}

$utf8 = [Text.UTF8Encoding]::new($false)
$loaderText = @"
-- Managed clink-posix-native loader. Disabling restores the previous px loader.
if not _G.clink_posix_native_loaded then
    _G.clink_posix_native_loaded = true
    local ok, err = pcall(dofile, [=[$forward/native-preview.lua]=])
    if ok then _G.clink_posix_loaded = true; _G.clink_posix_native_api = err
    else print('clink-native: '..tostring(err)) end
end
"@
$created = $false
$createdLoader = $false
try {
    New-Item -ItemType Directory -Path $runtime -ErrorAction Stop | Out-Null
    $created = $true
    Copy-Item -LiteralPath $sourceExe -Destination (Join-Path $runtime 'clink-posix-native.exe') -ErrorAction Stop
    Copy-Item -LiteralPath $sourceAdapter -Destination (Join-Path $runtime 'native-preview.lua') -ErrorAction Stop
    Copy-Item -LiteralPath $sourceManifest -Destination (Join-Path $runtime 'tools.tsv') -ErrorAction Stop
    [IO.File]::WriteAllText($loader, $loaderText, $utf8)
    $createdLoader = $true
    $receipt = [ordered]@{
        installed_at = (Get-Date).ToString('o')
        runtime = $runtime
        loader = $loader
        source_manifest = $sourceManifest
        version = '0.2.0'
        sha256 = [ordered]@{}
    }
    foreach ($name in @('clink-posix-native.exe', 'native-preview.lua', 'tools.tsv')) {
        $receipt.sha256[$name] = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $runtime $name)).Hash
    }
    $receipt.sha256['00-native.lua'] = (Get-FileHash -Algorithm SHA256 -LiteralPath $loader).Hash
    [IO.File]::WriteAllText((Join-Path $runtime 'installation.json'), ($receipt | ConvertTo-Json -Depth 5), $utf8)
    Write-Output "Installed native px adapter (nx alias retained): $loader"
    Write-Output "Runtime: $runtime"
} catch {
    if ($createdLoader -and (Test-Path -LiteralPath $loader)) { Remove-Item -LiteralPath $loader -Force }
    if ($created) {
        foreach ($name in @('installation.json', 'tools.tsv', 'native-preview.lua', 'clink-posix-native.exe')) {
            $path = Join-Path $runtime $name
            if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
        }
        Remove-Item -LiteralPath $runtime -ErrorAction SilentlyContinue
    }
    throw
}
