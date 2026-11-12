"""Run the exact current native engine pixel tests as an actual 32-bit process.

Maintainer-only: requires Rust's i686 target, a multilib C linker and 32-bit libc.
No game data, packages or player library are used.
"""
from pathlib import Path
import argparse,os,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rustc',default='rustc')
parser.add_argument('--linker',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
source=project/'src/native/pixel_tests.rs'
quoted=str(source).replace('\\','\\\\').replace('"','\\"')
with tempfile.TemporaryDirectory(prefix='gamepkg-native-pixels32-') as temporary:
    root=Path(temporary);harness=root/'pixels.rs';binary=root/'pixels'
    harness.write_text('const _: () = assert!(core::mem::size_of::<usize>() == 4);\ninclude!("'+quoted+'");\n')
    subprocess.run([args.rustc,'--edition=2021','--test','-O','--target','i686-unknown-linux-gnu','-C','linker='+args.linker,str(harness),'-o',str(binary)],check=True,timeout=120)
    assert binary.read_bytes()[:5]==b'\x7fELF\x01','Compiler did not produce ELF32'
    environment=dict(os.environ)
    if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
    subprocess.run([str(binary)],env=environment,check=True,timeout=30)
print('Actual ELF32 native pixel tests passed all current color/layout/format/length checks.')
