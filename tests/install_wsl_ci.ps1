# Test real file hashing and fail-closed download admission without installing WSL.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/../tools/install_wsl_ci.ps1"
$work = Join-Path ([IO.Path]::GetTempPath()) ("gamepkg-wsl-download-test-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
$script:Requests = 0
$script:Failures = @()
$script:RequestUris = @()
$script:Waits = @()
class GamePkgTestHttpFailure : System.Exception {
    [object]$Response
    GamePkgTestHttpFailure([int]$status) : base('simulated HTTP failure') {
        $this.Response = [pscustomobject]@{ StatusCode = $status }
    }
}
function Start-Sleep { param([int]$Seconds) $script:Waits += $Seconds }
function Invoke-WebRequest {
    param($Uri, $OutFile, $MaximumRetryCount, $RetryIntervalSec, $Headers)
    $script:Requests++
    $script:RequestUris += [string]$Uri
    if ($script:Failures.Count) {
        $status = $script:Failures[0]
        $script:Failures = @($script:Failures | Select-Object -Skip 1)
        [IO.File]::WriteAllBytes($OutFile, [byte[]]@(9))
        if ($status -eq 0) { throw [IO.IOException]::new('simulated permanent failure') }
        throw [GamePkgTestHttpFailure]::new($status)
    }
    [IO.File]::WriteAllBytes($OutFile, [byte[]]@(1, 2, 3, 4))
}
function Assert-Throws {
    param([scriptblock]$Operation, [string]$ExpectedMessage = '')
    $caught = $false
    try { & $Operation } catch {
        if ($ExpectedMessage -and $_.Exception.Message -ne $ExpectedMessage) { throw }
        $caught = $true
    }
    if (-not $caught) { throw 'Expected rejection did not occur.' }
}
$script:WslExists = $false
$script:WslImportFailure = $false
$script:WslKernelFailure = $false
$script:WslWrongRelease = $false
$script:WslKernelText = '6.18.54.1-microsoft-standard-WSL2'
$script:WslImports = 0
function Invoke-TestWsl {
    $global:LASTEXITCODE = 0
    if ($args[0] -eq '--import') {
        if ($args.Count -ne 6 -or $args[4] -ne '--version' -or $args[5] -ne '2') {
            throw 'Import did not explicitly request WSL 2.'
        }
        $script:WslImports++
        if ($script:WslImportFailure) { $global:LASTEXITCODE = 1 }
        else { $script:WslExists = $true }
    }
    elseif ($args[0] -eq '--set-version') {
        # Actual Windows 2022 WSL 3.0.3 reports an error for this no-op.
        $global:LASTEXITCODE = 1
        Write-Output 'The distribution is already the requested version.'
    }
    elseif ($args -contains '/bin/true') {
        if (-not $script:WslExists) { $global:LASTEXITCODE = 1 }
    }
    elseif ($args -contains '/usr/bin/cat') {
        if ($args -notcontains '--exec') { throw 'OS query passed through a shell.' }
        if ($script:WslWrongRelease) { Write-Output 'VERSION_ID="22.04"' }
        else { Write-Output 'VERSION_ID="24.04"' }
    }
    elseif ($args -contains '/usr/bin/uname') {
        if ($args -notcontains '--exec') { throw 'Kernel query passed through a shell.' }
        Write-Output $script:WslKernelText
        if ($script:WslKernelFailure) { $global:LASTEXITCODE = 1 }
    }
    else { throw "Unexpected WSL invocation: $args" }
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
    $script:Failures = @(504, 503)
    $script:RequestUris = @()
    $script:Waits = @()
    $retry = Join-Path $work 'retry'
    $release = 'https://github.com/vendor/sdk/releases/download/v1/component.zip'
    Get-VerifiedCiDownload $release $retry $hash
    if ($script:RequestUris.Count -ne 3 -or $script:RequestUris[0] -cne $release -or
        $script:RequestUris[1] -notlike ($release + '?gamepkg_ci_retry=*') -or
        $script:RequestUris[1] -ceq $script:RequestUris[2] -or ($script:Waits -join ',') -cne '1,2') {
        throw 'Transient download did not retry with bounded waits and fresh release URLs.'
    }
    foreach ($original in @('https://example.invalid/component', ($release + '?signature=keep'))) {
        $script:Failures = @(504)
        $script:RequestUris = @()
        Get-VerifiedCiDownload $original (Join-Path $work ([guid]::NewGuid().ToString('N'))) $hash
        if ($script:RequestUris.Count -ne 2 -or $script:RequestUris[0] -cne $original -or $script:RequestUris[1] -cne $original) {
            throw 'Retry altered an unrelated host or signed query.'
        }
    }
    foreach ($failure in @(403, 0)) {
        $script:Failures = @($failure)
        $before = $script:Requests
        $output = Join-Path $work ([guid]::NewGuid().ToString('N'))
        Assert-Throws { Get-VerifiedCiDownload $release $output $hash }
        if ($script:Requests -ne ($before + 1) -or (Test-Path $output) -or (Test-Path "$output.partial")) {
            throw 'Permanent failure retried or left unverified content.'
        }
    }
    $script:Failures = @(504, 504, 504, 504)
    $script:Waits = @()
    $before = $script:Requests
    $output = Join-Path $work 'exhausted'
    Assert-Throws { Get-VerifiedCiDownload $release $output $hash }
    if ($script:Requests -ne ($before + 4) -or ($script:Waits -join ',') -cne '1,2,4' -or
        (Test-Path $output) -or (Test-Path "$output.partial")) { throw 'Retry budget or partial cleanup failed.' }
    $oldRunnerEnvironment = $env:RUNNER_ENVIRONMENT
    try {
        $env:RUNNER_ENVIRONMENT = 'self-hosted'
        Assert-Throws { Install-GamePkgCiWsl }
    }
    finally { $env:RUNNER_ENVIRONMENT = $oldRunnerEnvironment }
    foreach ($existing in @($false, $true)) {
        $script:WslExists = $existing
        $script:WslImports = 0
        $directory = Join-Path $work ("test distribution " + $existing)
        New-Item -ItemType Directory -Path $directory | Out-Null
        Initialize-GamePkgCiWslDistribution 'Invoke-TestWsl' $directory (Join-Path $work 'root fs.wsl')
        $expectedImports = if ($existing) { 0 } else { 1 }
        if ($script:WslImports -ne $expectedImports) { throw 'Unexpected import of an existing distribution.' }
    }
    $script:WslExists = $false
    $script:WslImportFailure = $true
    $directory = Join-Path $work 'failed import'
    New-Item -ItemType Directory -Path $directory | Out-Null
    Assert-Throws { Initialize-GamePkgCiWslDistribution 'Invoke-TestWsl' $directory 'rootfs' } 'Verified Ubuntu WSL 2 import failed.'
    $script:WslImportFailure = $false
    $script:WslExists = $true
    $script:WslKernelFailure = $true
    Assert-Throws { Initialize-GamePkgCiWslDistribution 'Invoke-TestWsl' $directory 'rootfs' } 'Ubuntu 24.04 did not start on the WSL 2 kernel.'
    $script:WslKernelFailure = $false
    $script:WslKernelText = '4.4.0-Microsoft'
    Assert-Throws { Initialize-GamePkgCiWslDistribution 'Invoke-TestWsl' $directory 'rootfs' } 'Ubuntu 24.04 did not start on the WSL 2 kernel.'
    $script:WslKernelText = '6.18.54.1-microsoft-standard-WSL2'
    $script:WslWrongRelease = $true
    Assert-Throws { Initialize-GamePkgCiWslDistribution 'Invoke-TestWsl' $directory 'rootfs' } 'Ubuntu did not report the required 24.04 OS identity.'
    $global:LASTEXITCODE = 0
    Write-Output 'Verified download/cache admission, runner guard, fresh/existing WSL 2 and import/kernel failures passed.'

}
finally { Remove-Item -LiteralPath $work -Recurse -Force }
