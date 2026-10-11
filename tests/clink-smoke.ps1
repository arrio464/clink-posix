param([Parameter(Mandatory = $true)][string]$ClinkExe)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$work = Join-Path $root 'target/test-work'
if (-not (Test-Path -LiteralPath (Join-Path $work 'fixture.exe'))) {
    throw 'Run python tests/check.py first to build the fixture.'
}
$profile = Join-Path $work 'clink-profile'
$scripts = Join-Path $work 'clink-scripts'
New-Item -ItemType Directory -Force -Path $profile, $scripts | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText((Join-Path $profile 'clink_settings'), "clink.autostart = nul`nclink.logo = none`nclink.autoupdate = off`ncmd.get_errorlevel = True`n", $utf8)
$luaPath = (Join-Path $PSScriptRoot 'clink_smoke.lua').Replace('\', '/')
if ($luaPath.Contains("'")) { throw 'Smoke-test path may not contain a single quote.' }
[IO.File]::WriteAllText((Join-Path $scripts 'probe.lua'), "dofile('$luaPath')", $utf8)
& $env:COMSPEC /d /k "`"$ClinkExe`" inject --quiet --profile `"$profile`" --scripts `"$scripts`""
if ($LASTEXITCODE -ne 0) { throw 'Clink smoke session failed.' }
$report = Get-Content -LiteralPath (Join-Path $root 'clink-verification.txt')
if (($report | Where-Object { $_ -like 'PASS *' }).Count -ne 7 -or ($report | Where-Object { $_ -like 'FAIL *' })) {
    throw 'Clink smoke checks did not all pass.'
}
$report
