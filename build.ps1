$ErrorActionPreference = 'Stop'
$previousTarget = $env:CARGO_TARGET_DIR
try {
    $env:CARGO_TARGET_DIR = Join-Path $PSScriptRoot 'target'
    & cargo build --locked --offline --release --manifest-path (Join-Path $PSScriptRoot 'Cargo.toml')
    if ($LASTEXITCODE -ne 0) { throw 'Native build failed' }
    Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release\clink-posix-native.exe') -Destination $PSScriptRoot
    $manifest = Join-Path $PSScriptRoot 'tools.tsv'
    if (-not (Test-Path -LiteralPath $manifest)) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'tools.example.tsv') -Destination $manifest
    }
} finally {
    $env:CARGO_TARGET_DIR = $previousTarget
}
