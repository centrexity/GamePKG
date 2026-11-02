# Test real file hashing and fail-closed download admission without installing WSL.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/../tools/install_wsl_ci.ps1"
$work = Join-Path ([IO.Path]::GetTempPath()) ("gamepkg-wsl-download-test-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$script:Requests = 0
function Invoke-WebRequest {
    param($Uri, $OutFile, $MaximumRetryCount, $RetryIntervalSec)
    $script:Requests++
    [IO.File]::WriteAllBytes($OutFile, [byte[]]@(1, 2, 3, 4))
}
function Assert-Throws {
    param([scriptblock]$Operation)
    $caught = $false
    try { & $Operation } catch { $caught = $true }
    if (-not $caught) { throw 'Expected rejection did not occur.' }
}
try {
    $sample = Join-Path $work 'sample'
    [IO.File]::WriteAllBytes($sample, [byte[]]@(1, 2, 3, 4))
    $hash = (Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash
    $accepted = Join-Path $work 'accepted'
    Get-VerifiedCiDownload 'https://example.invalid/runtime' $accepted $hash
    if ((Get-FileHash -LiteralPath $accepted -Algorithm SHA256).Hash -ne $hash) { throw 'Accepted bytes changed.' }
    Get-VerifiedCiDownload 'https://example.invalid/runtime' $accepted $hash.ToLowerInvariant()
    if ($script:Requests -ne 1) { throw 'Verified cache unexpectedly downloaded again.' }
    $rejected = Join-Path $work 'rejected'
    Assert-Throws { Get-VerifiedCiDownload 'https://example.invalid/runtime' $rejected ('0' * 64) }
    if ((Test-Path $rejected) -or (Test-Path "$rejected.partial")) { throw 'Rejected download survived admission.' }
    [IO.File]::WriteAllBytes($accepted, [byte[]]@(9))
    Assert-Throws { Get-VerifiedCiDownload 'https://example.invalid/runtime' $accepted $hash }
    if ($script:Requests -ne 2) { throw 'Corrupt cache was silently replaced.' }
    $oldRunnerEnvironment = $env:RUNNER_ENVIRONMENT
    try {
        $env:RUNNER_ENVIRONMENT = 'self-hosted'
        Assert-Throws { Install-GamePkgCiWsl }
    }
    finally { $env:RUNNER_ENVIRONMENT = $oldRunnerEnvironment }
    Write-Output 'Verified download, cache reuse, corrupt download/cache rejection and disposable-runner guard passed.'
}
finally { Remove-Item -LiteralPath $work -Recurse -Force }
