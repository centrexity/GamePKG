"""Private real-window check of signed file order and unchanged picker indices."""
from pathlib import Path
import csv,io,json,os,shutil,subprocess,tempfile,time
from gui_test_support import start_display,game_window,click_text,choose_native_file
PROJECT=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-file-roles-gui-') as temporary:
    root=Path(temporary);library=root/'library';recipe=root/'recipe'
    shutil.copytree(PROJECT/'ports/doom',recipe)
    manifest=recipe/'manifest.toml';text=manifest.read_text().replace('Your DOOM.WAD or DOOM2.WAD (original PC IWAD)','Base Game')
    titles=['Patch Early','Patch Sprite','Patch Merge','Patch Late','Patch Mask','Patch Floor']
    extras=''.join(f'[[roms]]\nid="patch{i}"\ntitle="{title}"\n' for i,title in enumerate(titles))
    manifest.write_text('doom_wad_modes=["file","sprites","merge","file","nwt_merge","flats"]\n'+text.replace('[limits]',extras+'\n[limits]',1))
    config=root/'settings.toml';config.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
    env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',OMP_THREAD_LIMIT='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus');env.pop('WAYLAND_DISPLAY',None)
    key=root/'key';key.write_text('63'*32);package=root/'private.gamepkg'
    for arguments in [('pack',recipe,'-o',package,'--key',key),('install',package,'--trust-creator')]:
        subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,arguments)],env=env,check=True,capture_output=True,text=True,timeout=30)
    server=start_display(env,1500,2600);app=None
    try:
        app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
        window,_=game_window(env,'GamePKG');subprocess.run(['xdotool','windowsize',window,'1400','2500','windowfocus',window],env=env,check=True)
        click_text(window,env,'DOOM',root,timeout=20,max_y=800);time.sleep(.5)
        def words():
            subprocess.run(['xdotool','windowfocus',window,'mousemove','--window',window,'1300','60'],env=env,check=True);time.sleep(.2)
            shot=root/'screen.png';subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(shot)],env=env,check=True)
            enlarged=root/'enlarged.png';subprocess.run(['convert',str(shot),'-resize','200%',str(enlarged)],env=env,check=True)
            tsv=subprocess.check_output(['tesseract',str(enlarged),'stdout','--psm','11','tsv'],stderr=subprocess.DEVNULL,text=True)
            return [row for row in csv.DictReader(io.StringIO(tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if row['text'].strip()]
        def locate(rows,label):
            wanted=label.split()
            for i in range(len(rows)-len(wanted)+1):
                part=rows[i:i+len(wanted)]
                if [r['text'].casefold() for r in part]==[w.casefold() for w in wanted] and max(int(r['top']) for r in part)-min(int(r['top']) for r in part)<20:
                    return int(part[0]['top'])//2
            raise AssertionError(f'Missing visible title {label}: '+ ' '.join(r['text'] for r in rows))
        rows=words();section=locate(rows,'Your game files');rows=[row for row in rows if int(row['top'])//2>=section];ordered=['Base Game','Patch Merge','Patch Mask','Patch Floor','Patch Sprite','Patch Early','Patch Late']
        positions=[locate(rows,label) for label in ordered];assert positions==sorted(positions) and len(set(positions))==7,(positions," ".join(r["text"] for r in rows))
        text=' '.join(row['text'] for row in rows).casefold();assert 'recipe applies' in text and 'sprite replacements' in text,text
        top=locate(rows,'Patch Merge');buttons=[r for r in rows if r['text'].casefold()=='choose' and top<int(r['top'])//2<top+110];assert buttons,(title,top,' '.join(r['text'] for r in rows))
        button=min(buttons,key=lambda r:int(r['top']));x=(int(button['left'])+int(button['width'])//2)//2;y=(int(button['top'])+int(button['height'])//2)//2
        subprocess.run(['xdotool','mousemove','--window',window,str(x),str(y),'click','1'],env=env,check=True)
        selected=root/'selected-merge.wad';selected.write_bytes(b'private selector fixture')
        choose_native_file(env,'Choose Patch Merge',selected)
        deadline=time.monotonic()+10
        while time.monotonic()<deadline:
            path=library/'rom-selections.json'
            if path.exists():
                entries=json.loads(path.read_text())
                if entries:
                    saved=next(iter(entries.values()))
                    if saved[3]==str(selected):break
            time.sleep(.1)
        else:raise AssertionError('Reordered picker did not persist original manifest index 3')
        assert len(saved)==7 and all(not value for index,value in enumerate(saved) if index!=3),saved
        expected=list(saved)
        for original_index,title in [(0,'Base Game'),(5,'Patch Mask'),(6,'Patch Floor'),(2,'Patch Sprite'),(1,'Patch Early'),(4,'Patch Late')]:
            rows=words();section=locate(rows,'Your game files');rows=[row for row in rows if int(row['top'])//2>=section]
            top=locate(rows,title);buttons=[r for r in rows if r['text'].casefold()=='choose' and top<int(r['top'])//2<top+110];assert buttons,(title,top,' '.join(r['text'] for r in rows))
            button=min(buttons,key=lambda r:int(r['top']));x=(int(button['left'])+int(button['width'])//2)//2;y=(int(button['top'])+int(button['height'])//2)//2
            subprocess.run(['xdotool','mousemove','--window',window,str(x),str(y),'click','1'],env=env,check=True)
            selection=root/('choice-'+title.casefold().replace(' ','-')+'.wad');selection.write_bytes(f'private file {original_index}'.encode())
            choose_native_file(env,'Choose '+title,selection);expected[original_index]=str(selection)
            deadline=time.monotonic()+10
            while time.monotonic()<deadline:
                saved=next(iter(json.loads((library/'rom-selections.json').read_text()).values()))
                if saved==expected:break
                time.sleep(.1)
            else:raise AssertionError(f'Selection {original_index} changed another original manifest entry')
        app.terminate();app.wait(timeout=10)
        app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
        window,_=game_window(env,'GamePKG');subprocess.run(['xdotool','windowsize',window,'1400','2500','windowfocus',window],env=env,check=True)
        click_text(window,env,'DOOM',root,timeout=20,max_y=800);time.sleep(.5)
        rows=words();section=locate(rows,'Your game files');rows=[row for row in rows if int(row['top'])//2>=section]
        positions=[locate(rows,label) for label in ordered];assert positions==sorted(positions),positions
        for rank,original_index in enumerate([0,3,5,6,2,1,4]):
            title_y=positions[rank];bottom=positions[rank+1] if rank+1<len(positions) else title_y+100
            field=' '.join(row['text'] for row in rows if title_y<int(row['top'])//2<bottom)
            assert Path(expected[original_index]).name in field,(original_index,field)
        assert next(iter(json.loads((library/'rom-selections.json').read_text()).values()))==expected
        old=Path(expected[3]);moved=root/'moved-merge.wad';old.rename(moved)
        click_text(window,env,'Recheck readiness',root,timeout=15);time.sleep(.4)
        text=' '.join(row['text'] for row in words()).casefold()
        assert 'file for patch merge' in text and 'missing or unavailable' in text,text
        assert next(iter(json.loads((library/'rom-selections.json').read_text()).values()))==expected
        def click_merge_field():
            rows=words();section=locate(rows,'Your game files');rows=[row for row in rows if int(row['top'])//2>=section]
            top=locate(rows,'Patch Merge');found=[row for row in rows if Path(expected[3]).name in row['text'] and top<int(row['top'])//2<top+85];assert found
            row=found[0];x=(int(row['left'])+int(row['width'])//2)//2;y=(int(row['top'])+int(row['height'])//2)//2
            subprocess.run(['xdotool','mousemove','--window',window,str(x),str(y),'click','1'],env=env,check=True)
        click_merge_field();choose_native_file(env,'Choose Patch Merge',None);time.sleep(.3)
        assert next(iter(json.loads((library/'rom-selections.json').read_text()).values()))==expected,'Cancelling cleared remembered files'
        click_merge_field();choose_native_file(env,'Choose Patch Merge',moved);expected[3]=str(moved)
        deadline=time.monotonic()+10
        while time.monotonic()<deadline:
            if next(iter(json.loads((library/'rom-selections.json').read_text()).values()))==expected:break
            time.sleep(.1)
        else:raise AssertionError('Moved-file replacement did not preserve other selections')
        time.sleep(.3);text=' '.join(row['text'] for row in words()).casefold();assert 'missing or unavailable' not in text,text
        with moved.open('r+b') as file:file.truncate(64*1024*1024+1)
        click_text(window,env,'Recheck readiness',root,timeout=15);time.sleep(.3)
        text=' '.join(row['text'] for row in words()).casefold();assert 'file for patch merge' in text and '64 mib' in text,text
        moved.write_bytes(b'private selector fixture');click_text(window,env,'Recheck readiness',root,timeout=15);time.sleep(.3)
        text=' '.join(row['text'] for row in words()).casefold();assert '64 mib' not in text,text
        print('Real GUI: seven remembered fields, restart, named missing/oversized files, cancellation and moved-file replacement passed.')
    finally:
        if app is not None and app.poll() is None:app.terminate();app.wait(timeout=10)
        server.terminate();server.wait(timeout=10)
