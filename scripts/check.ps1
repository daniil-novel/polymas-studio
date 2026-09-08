param([ValidateSet('all','rust','ui','spec')][string]$Scope = 'all')
$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    if ($Scope -in @('all','rust')) {
        . (Join-Path $PSScriptRoot 'dev-env.ps1')
        cargo fmt --all --check
        if ($LASTEXITCODE -ne 0) { throw 'Rust formatting failed' }
        cargo test --workspace
        if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed' }
        cargo clippy --workspace --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw 'Rust lint failed' }
    }
    if ($Scope -in @('all','ui')) {
        npm run typecheck
        if ($LASTEXITCODE -ne 0) { throw 'TypeScript failed' }
        npm run lint
        if ($LASTEXITCODE -ne 0) { throw 'ESLint failed' }
        npm test
        if ($LASTEXITCODE -ne 0) { throw 'Frontend tests failed' }
        npm run build
        if ($LASTEXITCODE -ne 0) { throw 'UI build failed' }
    }
    if ($Scope -in @('all','spec')) {
        python scripts/validate_spec.py
        if ($LASTEXITCODE -ne 0) { throw 'Specification validation failed' }
    }
} finally { Pop-Location }
