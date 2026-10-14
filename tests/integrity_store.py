"""Restore exact signed shared assets from portable stores, then build and play."""
import os, shutil, subprocess, tempfile, sys, zipfile
from pathlib import Path
PROJECT = Path(__file__).resolve().parents[1]
CLI = PROJECT / 'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-integrity-store-') as directory:
    root = Path(directory)
    source = root / 'source'
    shutil.copytree(PROJECT / 'examples/minimal', source)
    library = root / 'library'
    env = dict(os.environ, GAMEPKG_HOME=str(library), GAMEPKG_CONFIG=str(root / 'settings.toml'))
    def run(*args, good=True):
        result = subprocess.run([str(CLI), *map(str,args)], env=env, capture_output=True, text=True, timeout=180)
        assert (result.returncode == 0) == good, result.stdout + result.stderr
        return result.stdout + result.stderr
    key = root / 'creator.key'
    run('keygen', '-o', key)
    package = root / 'original.gamepkg'
    run('pack', source, '-o', package, '--key', key, '--shared-assets')
    store = root / 'original.gamepkgstore'
    run('store-export', package, '-o', store)
    run('install', package, '--trust-creator')
    game = 'com.gamepkg.minimal'
    archive = library / (game + '.gamepkg')
    before = archive.read_bytes()
    blob, = (library / 'asset-pool').glob('*.blob')
    original = blob.read_bytes()
    blob.write_bytes(b'damaged asset')
    assert 'shared asset' in run('integrity-check', game, good=False)
    tampered = root / 'tampered.gamepkgstore'
    with zipfile.ZipFile(store) as original_store, zipfile.ZipFile(tampered,'w') as damaged_store:
        for name in original_store.namelist():
            data = original_store.read(name)
            damaged_store.writestr(name,b'x'*len(data) if name.startswith('blobs/') else data)
    assert 'signed reference' in run('integrity-fix',game,'--store',tampered,good=False)
    assert blob.read_bytes()==b'damaged asset' and archive.read_bytes()==before
    for kind in ['other-creator', 'same-version-different-source']:
        wrong = root / (kind + '.gamepkg')
        signing_key = key
        if kind == 'other-creator':
            signing_key = root / 'other.key'; run('keygen','-o',signing_key)
        else:
            (source / 'source/src/lib.rs').write_text('pub fn game_tick(_:u32,state:u64)->u64 { state + 9 }')
        wrong_store = root / (kind + '.gamepkgstore')
        # Export needs healthy source assets, but must leave the player's damaged blob untouched.
        creator = root / 'creator-library'
        creator.mkdir(exist_ok=True); (creator / 'asset-pool').mkdir(exist_ok=True)
        (creator / 'asset-pool' / blob.name).write_bytes(original)
        previous = env['GAMEPKG_HOME']; env['GAMEPKG_HOME'] = str(creator)
        run('pack', source, '-o', wrong, '--key', signing_key, '--shared-assets')
        run('store-export', wrong, '-o', wrong_store)
        env['GAMEPKG_HOME'] = previous
        assert 'exact signed game revision' in run('integrity-fix', game, '--store', wrong_store, good=False)
        assert blob.read_bytes() == b'damaged asset' and archive.read_bytes() == before
    assert 'restored 1 shared assets' in run('integrity-fix', game, '--store', store)
    assert blob.read_bytes() == original and archive.read_bytes() == before
    run('run', game, '--frames', '10000'); run('integrity-check', game)
    history_before = {p.name:p.read_bytes() for p in (library/'progress').glob('*.json')}
    assert history_before
    blob.unlink()
    assert 'restored 1 shared assets' in run('integrity-fix', game, '--store', store)
    assert 'restored 0 shared assets' in run('integrity-fix', game, '--store', store)
    external = root / 'private-file'; external.write_bytes(b'private content')
    blob.unlink(); blob.symlink_to(external)
    assert 'regular file' in run('integrity-fix', game, '--store', store, good=False)
    assert external.read_bytes() == b'private content' and blob.is_symlink()
    blob.unlink()
    archive.write_bytes(b'damaged archive too')
    assert 'restored 1 shared assets' in run('integrity-fix', game, '--store', store)
    assert archive.read_bytes() == before and blob.read_bytes() == original
    assert {p.name:p.read_bytes() for p in (library/'progress').glob('*.json')} == history_before
    run('run', game, '--frames', '3'); run('integrity-check', game)
    if '--gui' in sys.argv:
        import csv, io, time
        from gui_test_support import start_display
        blob.write_bytes(b'damaged again')
        archive.write_bytes(b'damaged archive again')
        env['LIBGL_ALWAYS_SOFTWARE'] = '1'; env.pop('WAYLAND_DISPLAY', None)
        (root / 'settings.toml').write_text('automatic_update_checks=false\n')
        display = start_display(env, 1600, 1100)
        app = subprocess.Popen([str(PROJECT / 'target/release/gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        try:
            for _ in range(100):
                found = subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
                if found.returncode == 0: break
                if app.poll() is not None: raise RuntimeError(app.stderr.read().decode())
                time.sleep(.1)
            else: raise AssertionError('GUI did not open')
            window = found.stdout.splitlines()[0]
            subprocess.run(['xdotool','windowsize',window,'1500','1050','windowfocus',window],env=env,check=True)
            time.sleep(.3)
            def screen():
                screenshot = root / 'repair-gui.png'
                subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(screenshot)],env=env,check=True)
                shutil.copy2(screenshot,PROJECT / 'target/integrity-store-gui.png')
                words = []
                for name,filters in [('plain',[]),('controls',['-colorspace','Gray','-negate','-threshold','55%'])]:
                    enlarged = root / (name+'.png')
                    subprocess.run(['convert',str(screenshot),*filters,'-resize','300%',str(enlarged)],env=env,check=True)
                    tsv = subprocess.check_output(['tesseract',str(enlarged),'stdout','--psm','11','tsv'],stderr=subprocess.DEVNULL,text=True)
                    words.extend(w for w in csv.DictReader(io.StringIO(tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if w['text'].strip())
                for w in words:
                    for field in ['left','top','width','height']: w[field] = str(int(w[field])//3)
                return ' '.join(w['text'] for w in words),words
            def click(w,dy=0):
                subprocess.run(['xdotool','mousemove','--window',window,str(int(w['left'])+int(w['width'])//2),str(int(w['top'])+int(w['height'])//2+dy),'click','1'],env=env,check=True)
            text,words = screen(); click(next(w for w in words if w['text']=='Local'),20)
            subprocess.run(['xdotool','type','--clearmodifiers',str(store)],env=env,check=True); time.sleep(.2)
            text,words = screen(); click(next(w for w in words if w['text']=='Repair'))
            end = time.monotonic()+30
            while True:
                text,_ = screen()
                if 'restored 1 shared assets' in text: break
                assert time.monotonic()<end,text
                time.sleep(.1)
            assert blob.read_bytes()==original and archive.read_bytes()==before
            run('run',game,'--frames','3')
            print('Desktop shared-asset repair passed: local store selection, actual repair button, rebuild and playable result.')
        finally:
            app.terminate(); app.wait(timeout=10)
            display.terminate(); display.wait(timeout=10)
print('Store integrity repair passed: damaged/missing assets, exact revision and creator checks, symlink rejection, archive recovery, isolated rebuild and playback.')
