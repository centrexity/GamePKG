# Maintainer-only bootstrap for the optional WSL compatibility tests.
# Versioned direct downloads avoid wsl --update's unauthenticated latest query.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

function Get-VerifiedCiDownload {
    param([string]$Uri, [string]$Destination, [string]$Sha256)
    if (Test-Path -LiteralPath $Destination) {
        if ((Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash -ieq $Sha256) {
            return
        }
        throw "Existing CI download failed SHA-256 verification: $Destination"
    }
    $partial = "$Destination.partial"
    try {
        Invoke-WebRequest -Uri $Uri -OutFile $partial -MaximumRetryCount 3 -RetryIntervalSec 5
        if ((Get-FileHash -LiteralPath $partial -Algorithm SHA256).Hash -ine $Sha256) {
            throw "CI download failed SHA-256 verification: $Uri"
        }
        Move-Item -LiteralPath $partial -Destination $Destination
    }
    finally {
        if (Test-Path -LiteralPath $partial) { Remove-Item -LiteralPath $partial }
    }
}

function Initialize-GamePkgCiWslDistribution {
    param([string]$Wsl, [string]$Work, [string]$Rootfs)
    # This exact name is how the pinned setup action detects an existing distro.
    & $wsl --distribution Ubuntu-24.04 --user root -- /bin/true
    if ($LASTEXITCODE -ne 0) {
        $distributionDirectory = Join-Path $work 'distribution'
        New-Item -ItemType Directory -Path $distributionDirectory | Out-Null
        & $wsl --import Ubuntu-24.04 $distributionDirectory $rootfs --version 2
        if ($LASTEXITCODE -ne 0) { throw 'Verified Ubuntu WSL 2 import failed.' }
    }
    # Import already selects version 2. Validate the running kernel instead
    # of converting an already-v2 distro, which WSL reports as an error.
    & $wsl --distribution Ubuntu-24.04 --user root -- /bin/sh -eu -c '. /etc/os-release; test "$VERSION_ID" = 24.04; uname -r | grep -i microsoft-standard-WSL2'
    if ($LASTEXITCODE -ne 0) { throw 'Ubuntu 24.04 did not start on the WSL 2 kernel.' }
}

function Install-GamePkgCiWsl {
    if (-not $IsWindows -or $env:GITHUB_ACTIONS -ne 'true' -or
        $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_ARCH -ne 'X64') {
        throw 'This installer is only for disposable GitHub-hosted Windows x64 test runners.'
    }
    if (-not $env:RUNNER_TEMP -or -not $env:GITHUB_PATH) {
        throw 'Missing GitHub runner temporary directory or path exchange.'
    }
    $work = Join-Path $env:RUNNER_TEMP ("gamepkg-wsl-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work | Out-Null
    $runtime = Join-Path $work 'wsl.3.0.3.0.x64.msi'
    # https://github.com/microsoft/WSL/releases/tag/3.0.3 (x64 asset digest)
    # https://releases.ubuntu.com/24.04.4/SHA256SUMS
    Get-VerifiedCiDownload 'https://github.com/microsoft/WSL/releases/download/3.0.3/wsl.3.0.3.0.x64.msi' `
        $runtime 'ae977286974255bd26bf925f00f40f5692c6b2084155cbc0563c09aea091f211'
    $rootfs = Join-Path $work 'ubuntu-24.04.4-wsl-amd64.wsl'
    Get-VerifiedCiDownload 'https://releases.ubuntu.com/24.04.4/ubuntu-24.04.4-wsl-amd64.wsl' `
        $rootfs '9b2f7730dc68227dd04a9f3e5eab86ad85caf556b8606ad94f1f29ff5c4fd3f5'
    $installer = Start-Process msiexec.exe -ArgumentList @('/i', "`"$runtime`"", '/quiet', '/norestart') -Wait -PassThru
    if ($installer.ExitCode -notin @(0, 3010)) {
        throw "WSL MSI installation failed: $($installer.ExitCode)"
    }
    $runtimeDirectory = Join-Path $env:ProgramFiles 'WSL'
    $wsl = Join-Path $runtimeDirectory 'wsl.exe'
    if (-not (Test-Path -LiteralPath $wsl)) { throw 'Verified WSL executable was not installed.' }
    & $wsl --version
    if ($LASTEXITCODE -ne 0) { throw 'Installed WSL runtime cannot report its version.' }
    Initialize-GamePkgCiWslDistribution $wsl $work $rootfs
    Add-Content -LiteralPath $env:GITHUB_PATH -Value $runtimeDirectory -Encoding utf8
    & $wsl --list --verbose
    if ($LASTEXITCODE -ne 0) { throw 'Unable to list the installed WSL distribution.' }
}

if ($MyInvocation.InvocationName -ne '.') { Install-GamePkgCiWsl }
