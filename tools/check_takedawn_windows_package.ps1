# Maintainer-only full native package gate. No ROM, WSL or Python is used.
# Use a disposable Windows runner: the first game build is substantial.
param(
    [Parameter(Mandatory = $true)][string]$Gamepkg,
    [Parameter(Mandatory = $true)][string]$Work
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'Run this gate on actual 64-bit Windows.' }
if (-not [Environment]::Is64BitProcess) { throw 'A 64-bit process is required.' }
$Gamepkg = (Resolve-Path -LiteralPath $Gamepkg).Path
$project = Split-Path -Parent $PSScriptRoot
$Work = [IO.Path]::GetFullPath($Work)
if (Test-Path -LiteralPath $Work) { throw 'Refusing to reuse an existing evidence directory.' }
[void][IO.Directory]::CreateDirectory($Work)
$evidence = Join-Path $Work 'evidence'
[void][IO.Directory]::CreateDirectory($evidence)
$library = Join-Path $Work 'Native library with spaces'
$key = Join-Path $Work 'private-creator.key'
$package = Join-Path $Work 'candidate.gamepkg'
$savedEnvironment = @{}
foreach ($name in @('GAMEPKG_HOME', 'GAMEPKG_CONFIG', 'PATH')) {
    $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name)
}

function Invoke-CheckedProcess {
    param([string]$Executable, [string[]]$CommandArguments, [string]$Stage,
          [switch]$ExpectFailure)
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $Executable
    $info.WorkingDirectory = $Work
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in $CommandArguments) { [void]$info.ArgumentList.Add($argument) }
    $stdout = [IO.File]::Create((Join-Path $evidence "$Stage.stdout.log"))
    $stderr = [IO.File]::Create((Join-Path $evidence "$Stage.stderr.log"))
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    $watch = [Diagnostics.Stopwatch]::StartNew()
    try {
        if (-not $process.Start()) { throw "Unable to start $Stage" }
        $outCopy = $process.StandardOutput.BaseStream.CopyToAsync($stdout)
        $errCopy = $process.StandardError.BaseStream.CopyToAsync($stderr)
        while (-not $process.WaitForExit(30000)) {
            Write-Host "$Stage remains live ($([int]$watch.Elapsed.TotalSeconds) seconds)."
        }
        [Threading.Tasks.Task]::WaitAll(@($outCopy, $errCopy))
        $exitCode = $process.ExitCode
    } finally {
        $stdout.Dispose()
        $stderr.Dispose()
        $process.Dispose()
    }
    Write-Host "$Stage exited $exitCode ($([int]$watch.Elapsed.TotalSeconds) seconds)."
    if (($exitCode -eq 0) -eq [bool]$ExpectFailure) {
        Get-Content -LiteralPath (Join-Path $evidence "$Stage.stderr.log") -Tail 30 | Write-Host
        throw "Unexpected exit status for $Stage"
    }
}

function Invoke-Gamepkg {
    param([string]$Stage, [string[]]$CommandArguments, [switch]$ExpectFailure)
    Invoke-CheckedProcess -Executable $Gamepkg -CommandArguments $CommandArguments `
        -Stage $Stage -ExpectFailure:$ExpectFailure
}

function Get-UnitInventory {
    $inventory = @{}
    $objects = @(Get-ChildItem -LiteralPath (Join-Path $library 'native-unit-cache/takedawn-windows') -Filter '*.o' -File)
    if ($objects.Count -ne 486) { throw "Expected all 486 compiled Rust units, found $($objects.Count)." }
    foreach ($object in $objects) {
        $hash = (Get-FileHash -LiteralPath $object.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        $receipt = [IO.File]::ReadAllText([IO.Path]::ChangeExtension($object.FullName, 'sha256'))
        if ($hash -cne $receipt) { throw 'A compiler checkpoint has an invalid receipt.' }
        $inventory[$object.Name] = "$hash/$($object.LastWriteTimeUtc.Ticks)"
    }
    return $inventory
}

try {
    $env:GAMEPKG_HOME = $library
    $env:GAMEPKG_CONFIG = Join-Path $Work 'settings.toml'
    # Compilation must discover its tools through the app's managed SDK. Python,
    # rustup, MSVC and development-shell tools are absent from the child PATH.
    $env:PATH = Join-Path $env:SystemRoot 'System32'
    [IO.File]::WriteAllText($env:GAMEPKG_CONFIG, "format_version = 1`nmemory_ceiling_mib = 4096`noptimize_builds = true`nautomatic_update_checks = false`ncheck_app_updates_on_startup = false`n")
    Invoke-Gamepkg 'keygen' @('keygen', '--output', $key)
    Invoke-Gamepkg 'pack' @('pack', (Join-Path $project 'ports/takedawn'), '--key', $key, '--output', $package)
    Invoke-Gamepkg 'verify' @('verify', $package)
    $packageHash = (Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash
    Invoke-Gamepkg 'install' @('install', $package, '--trust-creator')
    Invoke-Gamepkg 'before-build' @('integrity-check', 'com.gamepkg.takedawn')
    if ([IO.File]::ReadAllText((Join-Path $evidence 'before-build.stdout.log')) -notmatch 'not built') {
        throw 'The new private library unexpectedly contains a built game.'
    }
    Invoke-Gamepkg 'setup' @('setup-native-tools')
    Invoke-Gamepkg 'build' @('build', 'com.gamepkg.takedawn')
    Invoke-Gamepkg 'integrity' @('integrity-check', 'com.gamepkg.takedawn')
    Invoke-Gamepkg 'library' @('list')
    if ([IO.File]::ReadAllText((Join-Path $evidence 'library.stdout.log')) -notmatch 'Built \(native\)') {
        throw 'The library did not automatically recognize the native build.'
    }
    $games = @(Get-ChildItem -LiteralPath (Join-Path $library 'builds') -Filter 'takedawn-*.exe' -File -Recurse)
    if ($games.Count -ne 1) { throw 'Expected exactly one published native game executable.' }
    $game = $games[0].FullName
    $dll = Join-Path $games[0].DirectoryName 'SDL2.dll'
    $dllHash = (Get-FileHash -LiteralPath $dll -Algorithm SHA256).Hash
    $units = Get-UnitInventory
    # Damage only this disposable library's companion. The normal integrity and
    # repair actions must reject it, restore it and reuse every compiled unit.
    [IO.File]::WriteAllBytes($dll, [byte[]](0, 1, 2, 3))
    Invoke-Gamepkg 'damaged-integrity' @('integrity-check', 'com.gamepkg.takedawn') -ExpectFailure
    Invoke-Gamepkg 'repair' @('integrity-fix', 'com.gamepkg.takedawn', '--target', 'native')
    Invoke-Gamepkg 'repaired-integrity' @('integrity-check', 'com.gamepkg.takedawn')
    $repairedUnits = Get-UnitInventory
    foreach ($name in $units.Keys) {
        if (-not $repairedUnits.ContainsKey($name) -or $repairedUnits[$name] -cne $units[$name]) {
            throw "Repair did not preserve the verified compiler checkpoint $name"
        }
    }
    if ((Get-FileHash -LiteralPath $dll -Algorithm SHA256).Hash -cne $dllHash) {
        throw 'Repair did not restore the admitted SDL companion.'
    }
    if ((Get-FileHash -LiteralPath (Join-Path $library 'com.gamepkg.takedawn.gamepkg') -Algorithm SHA256).Hash -cne $packageHash) {
        throw 'Build or repair altered the signed installed package.'
    }
    @{
        format = 'gamepkg-native-windows-package-gate-v1'
        signed_install_build_integrity = $true
        library_native_ready = $true
        damaged_companion_rejected_and_repaired = $true
        rust_units_preserved_across_repair = $units.Count
        native_optimization = $true
        package_sha256 = $packageHash.ToLowerInvariant()
        executable_sha256 = (Get-FileHash -LiteralPath $game -Algorithm SHA256).Hash.ToLowerInvariant()
        executable_bytes = (Get-Item -LiteralPath $game).Length
        sdl_sha256 = $dllHash.ToLowerInvariant()
        original_media_used = $false
        original_gameplay_verified = $false
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $evidence 'result.json')
    Write-Host 'PASS: actual native Windows signed-package Build, readiness, integrity and repair; no media or gameplay verification.'
} finally {
    if (Test-Path -LiteralPath $key) { Remove-Item -LiteralPath $key -Force }
    foreach ($name in $savedEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name])
    }
}
