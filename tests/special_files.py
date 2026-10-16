"""Special host files must fail promptly instead of blocking loading or repairs."""
import hashlib, os, shutil, subprocess, tempfile
from pathlib import Path
PROJECT=Path(__file__).resolve().parents[1]
CLI=PROJECT/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-special-files-') as directory:
 root=Path(directory);source=root/'source';shutil.copytree(PROJECT/'examples/minimal',source)
 manifest=source/'manifest.toml'
 # Add a root-level field before the TOML tables.
 manifest.write_text('requires_rom=true\n'+(PROJECT/'examples/minimal/manifest.toml').read_text())
 library=root/'library';env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(*args,good=True,timeout=5):
  if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
  result=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=timeout)
  assert (result.returncode==0)==good,result.stdout+result.stderr
  return result.stdout+result.stderr
 key=root/'key';package=root/'game.gamepkg';run('keygen','-o',key);run('pack',source,'-o',package,'--key',key)
 run('install',package,'--trust-creator');run('build','com.gamepkg.minimal',timeout=180)
 game='com.gamepkg.minimal';rom=root/'rom';rom.write_bytes(b'ROM')
 cache=library/'builds'/hashlib.sha256(package.read_bytes()).hexdigest()/'game.wasm'
 receipt=cache.with_suffix('.sha256'); original=cache.read_bytes(); checksum=receipt.read_bytes()
 for target,valid in [(cache,original),(receipt,checksum)]:
  target.unlink();os.mkfifo(target)
  try:
   run('run',game,'--frames','1','--rom',rom,good=False)
   run('integrity-check',game,good=False)
   assert 'Not built' in run('list')
  finally:target.unlink();target.write_bytes(valid)
  outside=root/'linked-file';outside.write_bytes(valid);target.unlink();target.symlink_to(outside)
  try:run('run',game,'--frames','1','--rom',rom,good=False)
  finally:target.unlink();target.write_bytes(valid)
 fifo=root/'input-pipe';os.mkfifo(fifo)
 run('run',game,'--frames','1','--rom',fifo,good=False)
 run('integrity-fix',game,'--store',fifo,good=False)
 linked_rom=root/'linked-rom';linked_rom.symlink_to(rom)
 run('run',game,'--frames','1','--rom',linked_rom)
 oversized=root/'large-rom'
 with oversized.open('wb') as file:file.truncate(64*1024*1024+1)
 run('run',game,'--frames','1','--rom',oversized,good=False)
 with cache.open('wb') as file:file.truncate(32*1024*1024+1)
 run('run',game,'--frames','1','--rom',rom,good=False);cache.write_bytes(original)
 backup=library/'verified-packages'/(game+'.gamepkg');saved=backup.read_bytes()
 backup.unlink();os.mkfifo(backup);installed=library/(game+'.gamepkg');installed.write_bytes(b'damaged')
 try:run('integrity-fix',game,good=False)
 finally:backup.unlink();backup.write_bytes(saved);installed.write_bytes(package.read_bytes())
 # Rebuild must not block while comparing a damaged shader cache.
 shader=next(p for p in (library/'shader-builds').glob('*/*') if p.is_file() and p.suffix=='.shader')
 shader_bytes=shader.read_bytes();shader.unlink();os.mkfifo(shader)
 assert 'shader cache is damaged' in run('build',game,good=False,timeout=30)
 shader.unlink();shader.write_bytes(shader_bytes)
 outside_shader=root/'outside-shader';outside_shader.write_bytes(shader_bytes)
 shader.unlink();shader.symlink_to(outside_shader)
 assert 'shader cache is damaged' in run('build',game,good=False,timeout=30)
 assert outside_shader.read_bytes()==shader_bytes;shader.unlink();shader.write_bytes(shader_bytes)
 run('run',game,'--frames','1','--rom',rom)
print('Special file checks passed: FIFO and symlink caches/receipts, FIFO ROM/store/recovery, oversized files, explicit linked ROMs and normal playback.')
