"""Private original-engine multi-WAD qualification; never copies player IWADs."""
from pathlib import Path
import argparse, os, shutil, struct, subprocess, tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--wad',required=True,type=Path)
parser.add_argument('--baseline',action='store_true',help='also verify the unchanged single-WAD recipe in both selected targets')
parser.add_argument('--mode',choices=['file','merge','mixed_file_first','mixed_file_last','flats','sprites','both','nwt_merge'],default='file',help='signed original loading group arrangement for six patches')
parser.add_argument('--recovery-probe',action='store_true',help='verify missing resources preserve original saves and allow reopening')
parser.add_argument('--new-flat-probe',action='store_true',help='qualify insertion of a previously absent flat')
parser.add_argument('--nwt-probe',action='store_true',help='original NWT masking followed by file reload')
parser.add_argument('--sprite-probe',action='store_true',help='qualify visible weapon-sprite merging separately')
parser.add_argument('--flat-probe',action='store_true',help='qualify specialized flat inclusion/exclusion instead of the existing general mod workflow')
parser.add_argument('--target',choices=['native','wasm','both'],default='both')
args=parser.parse_args()
if args.recovery_probe and (args.flat_probe or args.sprite_probe or args.nwt_probe or args.new_flat_probe or args.mode not in ['flats','sprites','both','nwt_merge']):parser.error('recovery probe requires one replacement-only mode and no other probe')
if args.new_flat_probe and (args.flat_probe or args.sprite_probe or args.nwt_probe or args.mode not in ['merge','flats','sprites','both','nwt_merge']):parser.error('new-flat probe must run separately')
if args.nwt_probe:
    if args.flat_probe or args.sprite_probe:parser.error('NWT probe is separate from flat/sprite probes')
    args.mode='nwt_merge'
if args.flat_probe and args.mode not in ['merge','flats','sprites','both','nwt_merge']:parser.error('flat probe requires a merge-family mode')
if args.sprite_probe and (args.flat_probe or args.mode not in ['merge','sprites','both','flats']):parser.error('sprite probe requires merge/sprites/both/flats and excludes flat probe')
if not (args.flat_probe or args.sprite_probe or args.nwt_probe or args.new_flat_probe or args.recovery_probe) and args.mode in ['flats','sprites','both','nwt_merge']:parser.error('specialized modes require --flat-probe')
project=Path(__file__).resolve().parents[1]
cli=project/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-mods-') as temporary:
    root=Path(temporary); library=root/'library'; source=root/'recipe'
    shutil.copytree(project/'ports/doom',source)
    manifest=source/'manifest.toml'
    extras=''.join(f'[[roms]]\nid="patch{i}"\ntitle="Private patch {i}"\n' for i in range(6))
    modes=(["file"]+["merge"]*5 if args.mode=='mixed_file_first' else ["merge"]*5+["file"] if args.mode=='mixed_file_last' else [args.mode]*6)
    if args.flat_probe or args.sprite_probe or args.new_flat_probe or args.recovery_probe:modes=['file']*5+[args.mode]
    if args.nwt_probe:modes=['file']*4+['nwt_merge','file']
    mode_line='doom_wad_modes=['+','.join('\"'+mode+'\"' for mode in modes)+']\n'
    manifest.write_text(mode_line+manifest.read_text().replace('[limits]',extras+'\n[limits]',1))
    key=root/'key';key.write_text('62'*32)
    config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
    env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(config))
    def command(*arguments):
        result=subprocess.run(list(map(str,arguments)),env=env,capture_output=True,text=True,timeout=240)
        if result.returncode:
            raise RuntimeError(result.stdout[-6000:]+result.stderr[-6000:])
        if str(arguments[0]) == 'cargo' and '--exact' in arguments:
            if 'test result: ok. 1 passed; 0 failed; 0 ignored;' not in result.stdout:
                raise RuntimeError('Expected exactly one passing selected test:\n'+result.stdout[-6000:]+result.stderr[-6000:])
    package=root/'private.gamepkg'
    command(cli,'pack',source,'-o',package,'--key',key)
    command(cli,'install',package,'--trust-creator')
    library.mkdir(exist_ok=True)
    def room(start_x,floor=b'FLOOR0_1'):
        vertices=[(-128,-128),(-128,128),(128,128),(128,-128)]
        lines=b''.join(struct.pack('<7H',i,(i+1)%4,1,0,0,i,65535) for i in range(4))
        sides=b''.join(struct.pack('<hh8s8s8sh',0,0,b'-',b'-',b'STARTAN3',0) for _ in range(4))
        segs=b''.join(struct.pack('<6H',i,(i+1)%4,angle,i,0,0) for i,angle in enumerate([16384,0,49152,32768]))
        blockmap=struct.pack('<4h',-128,-128,3,3)+struct.pack('<9H',*([13]*9))+struct.pack('<6H',0,0,1,2,3,65535)
        return [(b'E1M1',b''),(b'THINGS',struct.pack('<5h',start_x,0,0,1,7)),
                (b'LINEDEFS',lines),(b'SIDEDEFS',sides),(b'VERTEXES',b''.join(struct.pack('<hh',*v) for v in vertices)),
                (b'SEGS',segs),(b'SSECTORS',struct.pack('<2H',4,0)),(b'NODES',b''),
                (b'SECTORS',struct.pack('<hh8s8s3h',0,128,floor,b'CEIL1_1',192,0,0)),
                (b'REJECT',b'\0'),(b'BLOCKMAP',blockmap)]
    for filename,index,start in [('a.wad',35,0),('b.wad',90,32)]:
        width,height=320,200
        post=bytes([0,height,0])+bytes([index])*height+bytes([0,255])
        picture=struct.pack('<4h',width,height,0,0)+b''.join(struct.pack('<I',8+width*4+x*len(post)) for x in range(width))+post*width
        for output,lumps in [(filename,[(b'TITLEPIC',picture)]),('map-'+filename,[(b'TITLEPIC',picture)]+room(start))]:
            payload=b'';directory=b''
            for name,content in lumps:
                directory+=struct.pack('<2I8s',12+len(payload),len(content),name)
                payload+=content
            patch=b'PWAD'+struct.pack('<2I',len(lumps),12+len(payload))+payload+directory
            (library/output).write_bytes(patch)
    lumps=room(0,b'PKGFLAT');payload=b'';directory=b''
    for name,content in lumps:
        directory+=struct.pack('<2I8s',12+len(payload),len(content),name);payload+=content
    (library/'map-newflat.wad').write_bytes(b'PWAD'+struct.pack('<2I',len(lumps),12+len(payload))+payload+directory)
    for filename,index in [('newflat-a.wad',35),('newflat-b.wad',90)]:
        lumps=[(b'F_START',b''),(b'PKGFLAT',bytes([index])*4096),(b'F_END',b'')]
        payload=b'';directory=b''
        for name,content in lumps:
            directory+=struct.pack('<2I8s',12+len(payload),len(content),name);payload+=content
        (library/filename).write_bytes(b'PWAD'+struct.pack('<2I',len(lumps),12+len(payload))+payload+directory)
    for filename,index in [('flat-a.wad',35),('flat-b.wad',90)]:
        lumps=[(b'F_START',b''),(b'FLOOR0_1',bytes([index])*4096),(b'F_END',b'')]
        payload=b'';directory=b''
        for name,content in lumps:
            directory+=struct.pack('<2I8s',12+len(payload),len(content),name);payload+=content
        (library/filename).write_bytes(b'PWAD'+struct.pack('<2I',len(lumps),12+len(payload))+payload+directory)
    for filename,index in [('sprite-a.wad',35),('sprite-b.wad',90)]:
        width,height=64,64
        post=bytes([0,height,0])+bytes([index])*height+bytes([0,255])
        sprite=struct.pack('<4h',width,height,-128,-106)+b''.join(struct.pack('<I',8+width*4+x*len(post)) for x in range(width))+post*width
        lumps=[(b'S_START',b''),(b'PISGA0',sprite),(b'S_END',b'')]
        payload=b'';directory=b''
        for name,content in lumps:
            directory+=struct.pack('<2I8s',12+len(payload),len(content),name);payload+=content
        (library/filename).write_bytes(b'PWAD'+struct.pack('<2I',len(lumps),12+len(payload))+payload+directory)
        nwt_lumps=[(b'SS_START',b''),(b'PISGA0',sprite),(b'S_END',b'')]
        payload=b'';directory=b''
        for name,content in nwt_lumps:
            directory+=struct.pack('<2I8s',12+len(payload),len(content),name);payload+=content
        (library/('nwt-'+filename)).write_bytes(b'PWAD'+struct.pack('<2I',len(nwt_lumps),12+len(payload))+payload+directory)

    targets=['wasm','native'] if args.target=='both' else [args.target]
    for target in targets:
        command(cli,'build','com.gamepkg.doom','--target',target)
        shutil.rmtree(library/'saves',ignore_errors=True)
        shutil.rmtree(library/'progress',ignore_errors=True)
        env.update(GAMEPKG_DOOM_MOD_ROOT=str(library),GAMEPKG_DOOM_TEST_WAD=str(args.wad.resolve()),GAMEPKG_DOOM_TEST_TARGET=target,GAMEPKG_DOOM_TEST_MODE=args.mode)
        test='original_missing_flat_preserves_saved_game_and_recovers' if args.recovery_probe else 'original_new_flat_insertion_requires_merge' if args.new_flat_probe else 'original_nwt_mask_and_file_reload_render_weapon' if args.nwt_probe else 'original_sprite_merge_modes_include_or_exclude_weapon_pixels' if args.sprite_probe else 'original_flat_merge_modes_include_or_exclude_floor_pixels' if args.flat_probe else 'original_patch_wad_order_title_and_gameplay'
        command('cargo','test','--locked','--release','--all-features','--test','doom_mods',test,'--','--ignored','--exact')
        if args.recovery_probe:print(f'{target}/{args.mode}: missing-resource failure preserved original save bytes; exact-position recovery and resumed controls passed.',flush=True)
        elif args.new_flat_probe:print(f'{target}/{args.mode}: new-flat insertion or original missing-resource rejection verified.',flush=True)
        elif args.nwt_probe:print(f'{target}: original NWT mask/file reload, repeatable replacement pixels, firing animation and exact idle return passed.',flush=True)
        elif args.sprite_probe:print(f'{target}/{args.mode}: actual sprite inclusion/exclusion, exact repeatability and stable original map coordinates passed.',flush=True)
        elif args.flat_probe:print(f'{target}/{args.mode}: actual flat inclusion/exclusion, exact repeatability and stable original map coordinates passed.',flush=True)
        else:print(f'{target}/{args.mode}: signed multi-WAD title/map overrides, exact starts, room rendering/collision, original gameplay/movement, save/reload and malformed patch rejection passed.',flush=True)

    if args.baseline:
        baseline=root/'baseline-library'
        env['GAMEPKG_HOME']=str(baseline)
        command(cli,'pack',project/'ports/doom','-o',root/'baseline.gamepkg','--key',key)
        command(cli,'install',root/'baseline.gamepkg','--trust-creator')
        for target in targets:
            command(cli,'build','com.gamepkg.doom','--target',target)
            shutil.rmtree(baseline/'saves',ignore_errors=True)
            shutil.rmtree(baseline/'progress',ignore_errors=True)
            env.update(GAMEPKG_DOOM_TEST_ROOT=str(baseline),GAMEPKG_DOOM_TEST_TARGET=target)
            command('cargo','test','--locked','--release','--all-features','--test','doom_play','real_wad_gameplay_audio_save_and_reload','--','--ignored','--exact')
            command('cargo','test','--locked','--release','--all-features','--test','doom_play','original_desktop_keyboard_mouse_binding_migration_and_quit','--','--ignored','--exact')
            command('cargo','test','--locked','--release','--all-features','--test','doom_play','original_linked_chaingun_sound_plays','--','--ignored','--exact')
            command('cargo','test','--locked','--release','--all-features','--test','doom_play','original_music_changes_maps_and_saved_map_resume','--','--ignored','--exact')
            if target=='wasm':command('cargo','test','--locked','--release','--all-features','--test','doom_play','stale_doom_engine_is_rejected_and_integrity_fix_rebuilds','--','--ignored','--exact')
            print(f'{target}: unchanged single-WAD original gameplay/audio/save/reload regression passed.',flush=True)
