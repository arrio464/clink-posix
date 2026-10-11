# Upgrade only receipt-verified files, keeping a complete recoverable backup.
$ErrorActionPreference = 'Stop'
$runtime = Join-Path $env:LOCALAPPDATA 'clink-posix-native'
$loader = Join-Path $env:USERPROFILE '.clink\00-native.lua'
$receiptPath = Join-Path $runtime 'installation.json'
$receipt = Get-Content -Raw -LiteralPath $receiptPath | ConvertFrom-Json
if ($receipt.runtime -ne $runtime -or $receipt.loader -ne $loader) { throw 'Managed installation path mismatch.' }
foreach ($name in @('clink-posix-native.exe','native-preview.lua','00-native.lua')) {
    $path = if ($name -eq '00-native.lua') { $loader } else { Join-Path $runtime $name }
    if ((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $receipt.sha256.$name) {
        throw "Managed file was edited; refusing to overwrite: $path"
    }
}
foreach ($name in @('clink-posix-native.exe','native-preview.lua')) {
    if (-not (Test-Path -LiteralPath (Join-Path $PSScriptRoot $name) -PathType Leaf)) { throw "Missing source: $name" }
}
$forward = $runtime.Replace('\','/')
if ($forward -match '[%!"\r\n]' -or $forward.Contains(']=]')) { throw 'Unsupported runtime transport path.' }
$backup = Join-Path $runtime ('backups\' + (Get-Date -Format 'yyyyMMdd-HHmmss-ffff'))
New-Item -ItemType Directory -Path $backup -ErrorAction Stop | Out-Null
foreach ($name in @('clink-posix-native.exe','native-preview.lua','installation.json','tools.tsv')) {
    Copy-Item -LiteralPath (Join-Path $runtime $name) -Destination (Join-Path $backup $name)
}
Copy-Item -LiteralPath $loader -Destination (Join-Path $backup '00-native.lua')
$loaderText = @"
-- Managed clink-posix-native loader. Disabling restores the previous px loader.
if not _G.clink_posix_native_loaded then
    _G.clink_posix_native_loaded = true
    local ok, err = pcall(dofile, [=[$forward/native-preview.lua]=])
    if ok then _G.clink_posix_loaded = true; _G.clink_posix_native_api = err
    else print('clink-native: '..tostring(err)) end
end
"@
$utf8 = [Text.UTF8Encoding]::new($false)
try {
    foreach ($name in @('clink-posix-native.exe','native-preview.lua')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $runtime $name)
    }
    [IO.File]::WriteAllText($loader,$loaderText,$utf8)
    $next = [ordered]@{
        installed_at = (Get-Date).ToString('o')
        version = '0.2.0'
        runtime = $runtime
        loader = $loader
        backup = $backup
        source_manifest = $receipt.source_manifest
        sha256 = [ordered]@{}
    }
    foreach ($name in @('clink-posix-native.exe','native-preview.lua','tools.tsv','00-native.lua')) {
        $path = if ($name -eq '00-native.lua') { $loader } else { Join-Path $runtime $name }
        $next.sha256[$name] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
    }
    [IO.File]::WriteAllText($receiptPath,($next | ConvertTo-Json -Depth 5),$utf8)
} catch {
    foreach ($name in @('clink-posix-native.exe','native-preview.lua','installation.json')) {
        Copy-Item -LiteralPath (Join-Path $backup $name) -Destination (Join-Path $runtime $name)
    }
    Copy-Item -LiteralPath (Join-Path $backup '00-native.lua') -Destination $loader
    throw
}
Write-Output "Upgraded px to native 0.2.0. Open a new CMD session. Backup: $backup"
