param([switch]$Dev,[switch]$Test)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$localTools = Join-Path (Split-Path $projectRoot -Parent) '.tools'
if (Test-Path (Join-Path $localTools 'cargo/bin/cargo.exe')) {
  $env:CARGO_HOME = Join-Path $localTools 'cargo'
  $env:RUSTUP_HOME = Join-Path $localTools 'rustup'
  $env:PATH = "$env:CARGO_HOME/bin;$env:PATH"
}
Push-Location $projectRoot
try {
  if ($Test) { Push-Location src-tauri; try { cargo test --locked; if ($LASTEXITCODE) { throw 'Rust 验证失败' } } finally { Pop-Location } }
  elseif ($Dev) { npm.cmd run desktop:dev }
  else { npm.cmd run desktop:build }
  if ($LASTEXITCODE) { throw '构建失败，请查看上方结果' }
} finally { Pop-Location }
