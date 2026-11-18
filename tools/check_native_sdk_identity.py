#!/usr/bin/env python3
"""Verify a maintainer app rlib matches current SDK sources before port checks."""
import argparse,hashlib,json,re,subprocess,tempfile
from pathlib import Path

def source_identity(app):
    module=app/'src/port_host/mod.rs';source=module.read_text();body=source.split('pub fn build_identity() -> String {',1)[1].split('\nuse std::',1)[0];items=body.split('for source in [',1)[1].split('] {',1)[0]
    pattern=re.compile(r'C_HEADER|CLIENT_SOURCE|include_str!\("([^"\\]+)"\)');cursor=0;paths=[]
    for match in pattern.finditer(items):
        assert not items[cursor:match.start()].strip(' \t\r\n,'),'unrecognized SDK identity expression'
        token=match.group();relative='../../include/gamepkg_port.h'if token=='C_HEADER'else'../../engines/native-services/client.rs'if token=='CLIENT_SOURCE'else match.group(1)
        paths.append((module.parent/relative).resolve(strict=True));cursor=match.end()
    assert not items[cursor:].strip(' \t\r\n,')and len(paths)>50,'incomplete SDK identity source list'
    digest=hashlib.sha256();files={}
    for path in paths:
        value=path.read_bytes();digest.update(len(value).to_bytes(8,'little'));digest.update(value);files[str(path.relative_to(app))]=hashlib.sha256(value).hexdigest()
    return digest.hexdigest(),files

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app-lib',type=Path,required=True);p.add_argument('--evidence',type=Path);a=p.parse_args();app=Path(__file__).resolve().parent.parent;lib=a.app_lib.resolve(strict=True);expected,files=source_identity(app)
    before=hashlib.sha256(lib.read_bytes()).hexdigest()
    with tempfile.TemporaryDirectory(prefix='gamepkg-sdk-identity-')as temp:
        root=Path(temp);source=root/'identity.rs';source.write_text('fn main(){println!("{}",gamepkg::port_host::build_identity());}\n');exe=root/'identity'
        subprocess.run(['rustc','--edition=2021','-D','warnings',str(source),'--extern','gamepkg='+str(lib),'-L','dependency='+str(lib.parent),'-o',str(exe)],check=True)
        actual=subprocess.check_output([str(exe)],text=True).strip()
    if actual!=expected:raise SystemExit('Stale app library: compiled SDK '+actual+' does not match current SDK sources '+expected+'. Rebuild the active app and select its actual emitted rlib before qualification.')
    assert hashlib.sha256(lib.read_bytes()).hexdigest()==before,'app library changed during identity check'
    assert source_identity(app)==(expected,files),'SDK sources changed during identity check'
    record={'schema':1,'sdk':actual,'app_lib':str(lib),'app_lib_sha256':before,'sources':files,'scope':'maintainer SDK/library consistency, not game admission or gameplay'}
    if a.evidence:a.evidence.write_text(json.dumps(record,indent=2)+'\n')
    print(actual)
if __name__=='__main__':main()
