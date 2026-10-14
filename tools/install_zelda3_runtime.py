"""Install a checksum-pinned optional Windows decoder runtime in a new folder."""
import argparse,hashlib,json,shutil,stat,tempfile,urllib.request,zipfile
from pathlib import Path,PurePosixPath
FILES = [{'filename': 'python-3.13.16-embed-amd64.zip', 'url': 'https://www.python.org/ftp/python/3.13.16/python-3.13.16-embed-amd64.zip', 'sha256': '97dae5274cc54867065e8d5a3226e48c35017ed332a0fdb0e27d5b5821961297', 'kind': 'python'}, {'filename': 'pillow-11.3.0-cp313-cp313-win_amd64.whl', 'url': 'https://files.pythonhosted.org/packages/23/85/397c73524e0cd212067e0c969aa245b01d50183439550d24d9f55781b776/pillow-11.3.0-cp313-cp313-win_amd64.whl', 'sha256': '0bce5c4fd0921f99d2e858dc4d4d64193407e1b99478bc5cacecba2311abde51', 'kind': 'wheel'}, {'filename': 'PyYAML-6.0.2-cp313-cp313-win_amd64.whl', 'url': 'https://files.pythonhosted.org/packages/fa/de/02b54f42487e3d3c6efb3f89428677074ca7bf43aae402517bc7cca949f3/PyYAML-6.0.2-cp313-cp313-win_amd64.whl', 'sha256': '8388ee1976c416731879ac16da0aff3f63b286ffdd57cdeb95f3f2e085687563', 'kind': 'wheel'}]

def install(destination):
    destination=Path(destination).resolve()
    if destination.exists():raise ValueError('Runtime destination must not exist')
    destination.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='zelda3-runtime-',dir=destination.parent) as temporary:
        stage=Path(temporary)/'runtime';stage.mkdir()
        for entry in FILES:
            with urllib.request.urlopen(entry['url'],timeout=60) as response:
                if not response.url.startswith(('https://www.python.org/','https://files.pythonhosted.org/')):raise ValueError('Unexpected runtime download origin')
                data=response.read(20000001)
            if len(data)>20000000 or hashlib.sha256(data).hexdigest()!=entry['sha256']:raise ValueError('Runtime download checksum mismatch')
            archive=Path(temporary)/entry['filename'];archive.write_bytes(data)
            output=stage if entry['kind']=='python' else stage/'Lib/site-packages'
            with zipfile.ZipFile(archive) as bundle:
                total=0
                for info in bundle.infolist():
                    path=PurePosixPath(info.filename)
                    if path.is_absolute() or '..' in path.parts or '\\' in info.filename or ':' in info.filename or stat.S_ISLNK(info.external_attr>>16):raise ValueError('Unsafe runtime archive path')
                    total+=info.file_size
                    if total>100000000:raise ValueError('Runtime archive too large')
                bundle.extractall(output)
        (stage/'python313._pth').write_text('python313.zip\n.\nLib/site-packages\n')
        (stage/'GAMEPKG-RUNTIME.json').write_text(json.dumps({'python':'3.13.16','pillow':'11.3.0','pyyaml':'6.0.2','downloads':FILES},indent=2)+'\n')
        stage.rename(destination)
    return destination

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination',type=Path,help='Usually bin/runtime/zelda3 beside the Windows host')
    args=parser.parse_args()
    print('Windows Zelda3 decoder runtime installed:',install(args.destination))
