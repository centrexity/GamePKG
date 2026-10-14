"""Check wrong-ROM rejection and header support without writing decoded resources."""
import argparse,hashlib,importlib.util,os,sys
from pathlib import Path
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('rom',type=Path);args=p.parse_args()
path=PROJECT/'engines/zelda3/adapters/rom_resources.py'
spec=importlib.util.spec_from_file_location('zelda3_decoder',path);decoder=importlib.util.module_from_spec(spec);spec.loader.exec_module(decoder)
original=args.rom.read_bytes();digest=hashlib.sha256(original).digest()
def audit(event,values):
    if event=='open' and values[2] & (os.O_CREAT|os.O_WRONLY|os.O_RDWR|os.O_TRUNC|os.O_APPEND):raise AssertionError('Decoder attempted file output')
    if event in ['os.mkdir','os.rename','os.remove']:raise AssertionError('Decoder attempted filesystem modification')
sys.addaudithook(audit)
plain=decoder.decode(original);headered=decoder.decode(bytes(512)+original)
assert plain==headered
for wrong in [b'',bytes(1048576),original[:-1]]:
    try:decoder.decode(wrong)
    except ValueError:pass
    else:raise AssertionError('Unsupported ROM was accepted')
assert hashlib.sha256(args.rom.read_bytes()).digest()==digest
print('ROM decoder: correct/headered equivalence, wrong/truncated rejection, unchanged source ROM and no writes passed.')
