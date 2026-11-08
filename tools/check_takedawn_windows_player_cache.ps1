# Maintainer regression: use a disposable fixture library with native Takedawn
# already built, empty shader cache, and mute_audio=true in its settings.
# Reads the owner's ISO in place; runs only the normal CLI player entry.
param(
    [Parameter(Mandatory=$true)][string]$Gamepkg,
    [Parameter(Mandatory=$true)][string]$Library,
    [Parameter(Mandatory=$true)][string]$Settings,
    [Parameter(Mandatory=$true)][string]$Rom,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference='Stop'
if($env:OS -ne 'Windows_NT' -or -not [Environment]::Is64BitProcess){throw 'Run on native 64-bit Windows.'}
foreach($p in @($Gamepkg,$Settings,$Rom)){if(-not (Test-Path -LiteralPath $p -PathType Leaf)){throw ('Missing fixture file: '+$p)}}
$Library=(Resolve-Path -LiteralPath $Library).Path
$Output=[IO.Path]::GetFullPath($Output)
if(Test-Path -LiteralPath $Output){throw 'Existing evidence must be inspected before reuse.'}
New-Item -ItemType Directory -Path $Output|Out-Null
if([IO.File]::ReadAllText($Settings) -notmatch '(?m)^mute_audio\s*=\s*true\s*$'){throw 'Fixture settings must explicitly mute automated audio.'}
$scopes=@(Get-ChildItem -LiteralPath (Join-Path $Library 'takedawn-data') -Directory)
if($scopes.Count -ne 1){throw 'Expected one prepared game-data scope.'}
$data=$scopes[0].FullName;$cache=Join-Path $data 'shader-cache'
if(Test-Path $cache){if(@(Get-ChildItem -LiteralPath $cache -File).Count){throw 'This regression requires an empty fixture cache; no existing cache is removed.'}}
$prior=@{}
foreach($n in @('GAMEPKG_HOME','GAMEPKG_CONFIG')){$prior[$n]=[Environment]::GetEnvironmentVariable($n)}
$report=[ordered]@{State='running';Scope='Normal protected CLI native player; same native executable; cold cache creation then warm reload; remembered ISO; no Python/WSL, manual controls or audio-quality claim';Library=$Library;Steps=@()}
function Publish {[IO.File]::WriteAllText((Join-Path $Output 'result.json'),($report|ConvertTo-Json -Depth 7),(New-Object Text.UTF8Encoding($false)))}
function Invoke-Player([string]$name,[string[]]$parts){
 $info=New-Object Diagnostics.ProcessStartInfo;$info.FileName=$Gamepkg;$info.WorkingDirectory=$Output;$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
 foreach($p in $parts){if($p.Contains('"') -or $p.EndsWith('\')){throw 'Unsupported fixture argument.'}}
 $info.Arguments=($parts|ForEach-Object {'"'+$_+'"'}) -join ' '
 $p=New-Object Diagnostics.Process;$p.StartInfo=$info
 $stdout=[IO.File]::Open((Join-Path $Output ($name+'.stdout.log')),'Create','Write','ReadWrite');$stderr=[IO.File]::Open((Join-Path $Output ($name+'.stderr.log')),'Create','Write','ReadWrite')
 $step=[ordered]@{Name=$name;State='starting'};$report.Steps+=,$step;Publish
 try{
  if(-not $p.Start()){throw 'Player start failed.'};$step.PID=$p.Id;$step.State='running';Publish
  $a=$p.StandardOutput.BaseStream.CopyToAsync($stdout);$b=$p.StandardError.BaseStream.CopyToAsync($stderr)
  # Observe the same process to completion. An observer timeout must not
  # restart a live player or mask its outcome.
  while(-not $p.WaitForExit(5000)){Publish}
  [Threading.Tasks.Task]::WaitAll(@($a,$b));$step.Exit=$p.ExitCode;$step.State='terminal';Publish
  if($p.ExitCode -ne 0){throw ($name+' exited '+$p.ExitCode)}
 }finally{$stdout.Dispose();$stderr.Dispose();$p.Dispose()}
}
function Capture-Cache([string]$name){
 $files=@(Get-ChildItem -LiteralPath $cache -File)
 foreach($required in @('vk_spirv.bin','vk_pipelines.bin')){if(@($files|Where-Object {$_.Name -eq $required -and $_.Length -gt 0}).Count -ne 1){throw ('Persistent shader cache missing: '+$required)}}
 $meta=@($files|ForEach-Object {[ordered]@{Name=$_.Name;Bytes=$_.Length;SHA256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}})
 $log=Join-Path $data 'saves\logs\run.log';Copy-Item -LiteralPath $log -Destination (Join-Path $Output ($name+'.runtime.log'))
 $text=[IO.File]::ReadAllText($log)
 if($text.Contains('pcache: no disk cache')){throw 'Normal player disabled its disk cache.'}
 return [ordered]@{Files=$meta;StartupLog=$text}
}
try{
 $env:GAMEPKG_HOME=$Library;$env:GAMEPKG_CONFIG=$Settings;Publish
 Invoke-Player 'ready-before' @('list')
 if(-not [IO.File]::ReadAllText((Join-Path $Output 'ready-before.stdout.log')).Contains('Built (native)')){throw 'Native fixture must already be built.'}
 Invoke-Player 'cold' @('run','com.gamepkg.takedawn','--frames','1200','--rom',$Rom)
 $cold=Capture-Cache 'cold';$report.ColdCache=$cold.Files;Publish
 Invoke-Player 'warm' @('run','com.gamepkg.takedawn','--frames','1200')
 $warm=Capture-Cache 'warm';$report.WarmCache=$warm.Files
 if($warm.StartupLog -notmatch 'pcache:.*loaded: [1-9][0-9]* records'){throw 'Second normal launch did not load cached pipeline keys.'}
 if($warm.StartupLog -notmatch 'pre-warmed [1-9][0-9]* vs|pre-warmed [0-9]+ vs, [1-9][0-9]* fs|pre-warmed [0-9]+ vs, [0-9]+ fs, [1-9][0-9]* pipelines'){throw 'Second launch did not pre-warm cached shaders/pipelines.'}
 Invoke-Player 'integrity-after' @('integrity-check','com.gamepkg.takedawn')
 $report.Result='pass-persistent-cold-and-warm-cache';$report.RememberedRom=$true
}catch{$report.Result='failure';$report.Error=$_.ToString();$report.Stack=$_.ScriptStackTrace;throw}
finally{$report.State='terminal';Publish;foreach($n in $prior.Keys){[Environment]::SetEnvironmentVariable($n,$prior[$n])}}
