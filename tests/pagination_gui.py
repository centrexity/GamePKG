"""Page real library rows and verify search/filter resets, sizes and refresh clamping."""
from pathlib import Path
import os, re, shutil, subprocess, tempfile, time
from gui_test_support import start_display, game_window, click_text
PROJECT = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-pagination-') as directory:
    root=Path(directory);home=root/'library';config=root/'settings.toml'
    config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\nrenderer="opengl"\n')
    env=dict(os.environ,GAMEPKG_HOME=str(home),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
    def cli(*args):subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,args)],env=env,check=True,capture_output=True,text=True)
    source=root/'source';shutil.copytree(PROJECT/'examples/minimal',source)
    manifest=source/'manifest.toml';manifest.write_text('title="Installed Puzzle"\n'+manifest.read_text())
    key=root/'key';key.write_text('42'*32);package=root/'game.gamepkg'
    cli('pack',source,'-o',package,'--key',key);cli('install',package,'--trust-creator')
    catalog=root/'pages.gamepkglist';catalog.write_text('format_version=1\n'+''.join(f'[[games]]\nid="com.example.page{i:03}"\ntitle="Catalog Game {i:03}"\nversion="1.0.0"\nrequires_rom={str(i%2==0).lower()}\ndownload_url="https://example.invalid/{i}.gamepkg"\n' for i in range(51)))
    cli('import-list',catalog)
    display=start_display(env,1440,1100)
    app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
    try:
        window,_=game_window(env,'GamePKG')
        subprocess.run(['xdotool','windowsize',window,'1360','1000'],env=env,check=True);time.sleep(.4)
        def click(text,top=True):
            click_text(window,env,text,root,max_y=650 if top else None);time.sleep(.4)
        def text():
            image=root/'screen.png';subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(image)],env=env,check=True)
            subprocess.run(['convert',str(image),'-resize','200%',str(root/'ocr.png')],env=env,check=True)
            return ' '.join(subprocess.check_output(['tesseract',str(root/'ocr.png'),'stdout','--psm','11'],stderr=subprocess.DEVNULL,text=True).casefold().split())
        def page(number,pages,title):
            screen=text();assert re.search(fr'page{number}[o0]+f{pages}(?!\d)', re.sub(r'[^a-z0-9]', '', screen)),screen
            assert title.casefold() in screen,screen
        def search(query):
            x,y=click_text(window,env,'Search',root,click=False,max_y=400)
            subprocess.run(['xdotool','mousemove','--window',window,str(x+130),str(y),'click','1','key','--clearmodifiers','ctrl+a'],env=env,check=True)
            subprocess.run(['xdotool','type','--clearmodifiers','--delay','0',query],env=env,check=True);time.sleep(.4)
        page(1,3,'Installed Puzzle')
        click('Next page');page(2,3,'Catalog Game 019')
        click('Next page');page(3,3,'Catalog Game 039')
        click('Next page');page(3,3,'Catalog Game 039')  # Disabled at the end.
        click('Previous page');page(2,3,'Catalog Game 019')
        search('Game 050');assert 'catalog game 050' in text();assert 'page 2' not in text()
        click('Clear');page(1,3,'Installed Puzzle')
        click('Next page');click('Needs ROM');page(1,2,'Catalog Game 000')
        click('All games');page(1,3,'Installed Puzzle')
        click('20 per page');click('10 per page');page(1,6,'Installed Puzzle')
        # Use the bottom pager and confirm it returns the page to the top.
        subprocess.run(['xdotool','mousemove','--window',window,'900','700','click','--repeat','70','--delay','5','5'],env=env,check=True);time.sleep(.4)
        click('Next page',top=False);page(2,6,'Catalog Game 009');assert 'your games' in text()
        click('10 per page');click('50 per page');page(1,2,'Installed Puzzle')
        click('Next page');page(2,2,'Catalog Game 049')
        subprocess.run(['xdotool','windowsize',window,'620','800'],env=env,check=True);time.sleep(.4)
        page(2,2,'Catalog Game 049')
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(PROJECT/'target/pagination-compact.png')],env=env,check=True)
        subprocess.run(['xdotool','windowsize',window,'1360','1000'],env=env,check=True);time.sleep(.4)
        # Model an external catalog update that removes entries, then refresh.
        state=home/'catalog.gamepkglist';state.write_text('[[games]]'.join(state.read_text().split('[[games]]')[:3]))
        click('Refresh library',top=False)
        screen=text();assert 'installed puzzle' in screen and 'catalog game 000' in screen,screen
        assert 'page 2' not in screen and 'no games match' not in screen,screen
        search('missing-entry');assert 'no games match your search and filters' in text()
        click('Clear');assert 'installed puzzle' in text()
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(PROJECT/'target/pagination-library.png')],env=env,check=True)
        print('Pagination passed: first/middle/last pages, disabled boundary, sizes, bottom navigation/scroll reset, global search, filter reset, refresh shrink, and empty results.')
    finally:
        app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
