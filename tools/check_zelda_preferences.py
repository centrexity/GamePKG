"""Private native/Wasm launch checks for saved player display overrides.

Builds each target once, then changes preferences without rebuilding or modifying
its signed recipe. The explicitly selected ROM is read in place.
"""
from pathlib import Path
import argparse
import os
import shutil
import subprocess
import tempfile

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rom',type=Path,required=True)
parser.add_argument('--sysroot',type=Path,required=True)
parser.add_argument('--slang-root',type=Path,required=True)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-display-preferences-') as temporary:
    root=Path(temporary);recipe=root/'recipe';library=root/'library';settings=root/'settings.toml'
    settings.write_text('automatic_update_checks=false\n')
    shutil.copytree(project/'ports/zelda3',recipe)
    with (recipe/'manifest.toml').open('a') as manifest:
        manifest.write('\n[zelda_video]\nextra_width=32\nextend_height=false\n')
    environment=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(settings),
        GAMEPKG_SLANG_ROOT=str(args.slang_root.resolve()),GAMEPKG_ZELDA3_SYSROOT=str(args.sysroot.resolve()),
        GAMEPKG_ZELDA3_TEST_ROOT=str(library),GAMEPKG_ZELDA3_TEST_ROM=str(args.rom.resolve(strict=True)),
        GAMEPKG_BUNDLED_AUDIO_DECODER=str(project/'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm'),
        GAMEPKG_BUNDLED_AUDIO_CONVERTER=str(project/'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm'))
    cli=project/'target/release/gamepkg'
    def run(*command):subprocess.run(list(map(str,command)),cwd=project,env=environment,check=True,timeout=600)
    run(cli,'keygen','-o',root/'key')
    run(cli,'pack',recipe,'-o',root/'source.gamepkg','--key',root/'key')
    source=(root/'source.gamepkg').read_bytes()
    run(cli,'install',root/'source.gamepkg','--trust-creator')
    for target in ['native','wasm']:run(cli,'build','com.gamepkg.zelda3','--target',target)
    baseline=None
    for index,(preset,tall,sprites,dim,width,height) in enumerate([
        ('default',False,False,False,320,224),('wide',False,True,False,398,224),('widest',True,True,False,448,240),
        ('original',False,True,False,256,224),('wide',True,False,False,426,240),('default',False,False,False,320,224),('original',False,False,False,256,224),('original',False,False,True,256,224),('original',False,False,False,256,224)]):
        command=[cli,'configure-display','com.gamepkg.zelda3','--preset',preset]
        if tall:command.append('--extended-height')
        if sprites:command.append('--extra-sprites')
        if dim:command.append('--dim-flashes')
        run(*command)
        environment.update(GAMEPKG_ZELDA3_VIDEO_WIDTH=str(width),GAMEPKG_ZELDA3_VIDEO_HEIGHT=str(height))
        fingerprints=[]
        for target in ['native','wasm']:
            output=root/f'fingerprint-{index}-{target}'
            environment.update(GAMEPKG_ZELDA3_TEST_TARGET=target,GAMEPKG_ZELDA3_VIDEO_FINGERPRINT=str(output))
            run('cargo','test','--locked','--release','--all-features','--test','zelda3_play','actual_zelda3_video_dimensions_aspect_and_pixels','--','--ignored','--exact')
            fingerprints.append(output.read_text())
        assert fingerprints[0]==fingerprints[1],(preset,tall,'pixel mismatch')
        if index==0:baseline=fingerprints[0]
        if index==6:flash_baseline=fingerprints[0]
        if index==7:assert fingerprints[0]!=flash_baseline,'Dim flashes did not change actual intro pixels'
        if index==8:assert fingerprints[0]==flash_baseline,'Disabling dim flashes did not restore original pixels'
        if index==5:assert fingerprints[0]==baseline,'Package default did not restore original recipe pixels'
        assert (root/'source.gamepkg').read_bytes()==source,'Signed source was modified'
        print(f'Saved {preset}, extended={tall}, extra_sprites={sprites}, dim_flashes={dim}: both targets launch at {width}x{height} with identical pixels and no rebuild.',flush=True)
    run(cli,'integrity-check','com.gamepkg.zelda3')
