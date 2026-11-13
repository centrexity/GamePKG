"""Maintainer-only FFmpeg graph exporter; player builds execute reviewed Ninja data."""
from pathlib import Path
import hashlib, json, re, shlex, subprocess, tarfile
import argparse
import tempfile


SOURCE_SHA256 = '8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e'


def decoder_fragment(source, archive_path, build_log, out):
    assert hashlib.sha256(archive_path.read_bytes()).hexdigest() == SOURCE_SHA256
    configuration = (source/'config.h').read_text()
    for name in ['CONFIG_GPL', 'CONFIG_VERSION3', 'CONFIG_NONFREE', 'CONFIG_NETWORK',
                 'CONFIG_AVDEVICE', 'CONFIG_AVFILTER', 'HAVE_INLINE_ASM', 'HAVE_X86ASM']:
        assert re.search(r'^#define '+name+r' 0$', configuration, re.M), name
    prefix = 'build/linux/third_party/ffmpeg'
    obj = 'build/linux/ffmpeg-obj'
    official = {}
    with tarfile.open(archive_path) as archive:
        for member in archive:
            name = '/'.join(member.name.split('/')[1:])
            if name and member.isfile(): official[name] = archive.extractfile(member).read()
            elif name and not member.isdir(): raise ValueError(('unexpected archive type', name))
    commands = [shlex.split(x) for x in build_log.read_text().splitlines()
                if x.startswith('/usr/bin/clang-18 ')]
    assert len(commands) == 229
    required = set()
    records = []
    graph = ['# FFmpeg 9.0.2 native-i686; no player configure/Python.\n']
    for args in commands:
        output = args[args.index('-o')+1]
        csource = args[-1]
        assert csource.endswith('.c') and output.endswith('.o')
        dep = (source/output).with_suffix('.d').read_text().replace('\\\n', ' ')
        dependencies = shlex.split(dep.split(':', 1)[1])
        required.update(dependencies)
        rewritten = []
        for flag in args[1:args.index('-MMD')]:
            if flag.startswith('--sysroot='): flag = '--sysroot=build/linux/third_party/sysroot'
            elif flag.startswith('-I'):
                path = flag[2:].removeprefix('./').rstrip('.')
                flag = '-I'+prefix+('/'+path if path else '')
            rewritten.append(flag)
        implicit = [prefix+'/'+name for name in dependencies if name != csource]
        graph += [f'build {obj}/{output}: linux_cc {prefix}/{csource} | '+
                  ' '.join(implicit)+' build/gamepkg-compiler.stamp\n',
                  '  cflags = '+shlex.join(rewritten)+'\n']
        records.append(dict(source=csource, output=output, flags=rewritten, dependencies=dependencies))
    headers = []
    for name in sorted(required):
        data = (source/name).read_bytes()
        if name in official:
            assert data == official[name], 'Modified official source: '+name
            continue
        assert Path(name).suffix in ['.h', '.c'], name
        if name == 'config.h':
            data = re.sub(rb'^#define FFMPEG_CONFIGURATION .*$',
                b'#define FFMPEG_CONFIGURATION "GamePKG reviewed Bink-only native-i686 static build; no network/protocols/programs/assembly"', data, flags=re.M)
            data = re.sub(rb'^#define (FFMPEG_DATADIR|AVCONV_DATADIR) .*$', rb'#define \1 ""', data, flags=re.M)
        assert str(source).encode() not in data and not re.search(rb'/(?:home|media|Users|tmp)/', data), name
        destination = out/'headers'/name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        headers.append(dict(input='ffmpeg/'+name, output=prefix+'/'+name,
                            sha256=hashlib.sha256(data).hexdigest()))
    graph += ['rule gamepkg_ffmpeg_archive\n  command = rm -f $out && llvm-ar rcsD $out $in\n  description = FFMPEG ARCHIVE $out\n']
    libraries = []
    for part in ['avformat', 'avcodec', 'swscale', 'swresample', 'avutil']:
        archive = source/f'lib{part}/lib{part}.a'
        # GNU Make emits the response-file contents even in dry-run mode. Never
        # derive paths from ar's basenames: x86/scalar objects can share names.
        subprocess.run(['make', '-n', '-B', str(archive.relative_to(source))], cwd=source,
                       stdout=subprocess.DEVNULL, check=True)
        objects = shlex.split(Path(str(archive)+'.objs').read_text())
        known = {x['output'] for x in records}
        assert objects and len(set(objects)) == len(objects) and set(objects) <= known
        members = subprocess.check_output(['llvm-ar-18', 't', str(archive)], text=True).splitlines()
        assert [Path(x).name for x in objects] == members
        library = f'{obj}/lib{part}.a'
        libraries.append(library)
        graph += [f'build {library}: gamepkg_ffmpeg_archive '+' '.join(obj+'/'+x for x in objects)+'\n']

    manifest = dict(download=dict(urls=['https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz'],
        sha256='8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e',
        format='tar-xz', output=prefix, strip=1),
        dependency_tree=dict(root=prefix, files=[dict(path=name, sha256=hashlib.sha256(data).hexdigest())
                                               for name, data in sorted(official.items()) if not name.startswith('tests/maps/')]),
        headers=headers, libraries=libraries, compiler_count=len(records), generated_header_count=len(headers))
    return manifest, ''.join(graph), records


def extend(source, output, ffmpeg_source, archive_path, build_log):
    """Extend a newly exported base plan; source must already have patch 0014."""
    with tempfile.TemporaryDirectory(prefix='gamepkg-movie-export-') as temporary:
        scratch = Path(temporary)
        fragment, extra_graph, records = decoder_fragment(ffmpeg_source, archive_path, build_log, scratch)
        plan = json.loads((output/'plan.json').read_text())
        assert not any('ffmpeg' in x['output'] for x in plan['downloads']), 'Export the base plan first'
        plan['downloads'].append(fragment['download'])
        plan['dependency_trees'].append(fragment['dependency_tree'])
        plan['skip_archive_paths'].append('tests/maps')
        for header in fragment['headers']:
            plan['headers'].append({key: header[key] for key in ['input', 'output']})
            destination = output/header['input']
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((scratch/'headers'/header['input'].removeprefix('ffmpeg/')).read_bytes())
        graph = (output/'build.ninja').read_text()
        pattern = r'(?ms)^build build/linux/obj/port/linux/src/bink_null\.o:.*?(?=^build )'
        match = re.search(pattern, graph)
        assert match, 'Base graph must contain the original backend'
        codec = 'build/linux/third_party/ffmpeg'
        headers = sorted({codec+'/'+name for record in records for name in record['dependencies'] if name.endswith('.h')})
        implicit = ['port/linux/src/gamepkg_bink_ffmpeg.inc', 'port/linux/src/xiso.h', 'port/linux/src/port_config.h',
                    'build/linux/third_party/stamp', 'build/gamepkg-compiler.stamp', *headers]
        flags = ['--target=i686-linux-gnu', '-m32', '-mstackrealign', '-msse2', '-fms-extensions', '-std=gnu17',
                 '-D_GNU_SOURCE', '-D_FILE_OFFSET_BITS=64', '-DGAMEPKG_BINK_FFMPEG=1', '-O2', '-g', '-ffp-contract=off',
                 '-Wall', '-Wextra', '-Werror', '--sysroot=build/linux/third_party/sysroot',
                 '-Ibuild/linux/third_party/SDL3/include', '-Iport/linux/src', '-I'+codec]
        node = 'build build/linux/obj/port/linux/src/gamepkg_bink_bridge.o: linux_cc port/linux/src/gamepkg_bink_bridge.c | '+' '.join(implicit)+'\n'
        node += '  cflags = '+' '.join(flags)+'\n'
        graph = graph[:match.start()]+node+graph[match.end():]
        graph = graph.replace('build/linux/obj/port/linux/src/bink_null.o', 'build/linux/obj/port/linux/src/gamepkg_bink_bridge.o')
        pattern = r'(?ms)^build build/linux/obj/source/bink/bink_playback\.o:.*?(?=^build )'
        match = re.search(pattern, graph)
        assert match
        block = match.group().replace('  cflags = ', '  cflags = -DGAMEPKG_BINK_FFMPEG=1 ', 1)
        graph = graph[:match.start()]+block+graph[match.end():]
        pattern = r'(?ms)^build build/linux/obj/port/linux/src/xiso\.o:.*?(?=^build )'
        match = re.search(pattern, graph)
        assert match
        block = match.group().replace('    build/gamepkg-compiler.stamp',
            '    port/linux/src/gamepkg_xiso_view.inc build/gamepkg-compiler.stamp', 1)
        assert 'gamepkg_xiso_view.inc' in block
        graph = graph[:match.start()]+block+graph[match.end():]
        libraries = fragment['libraries']
        graph = graph.replace('tools/linux_link_check.py build/linux/sdl3-build/libSDL3.so.0',
            'tools/linux_link_check.py build/linux/sdl3-build/libSDL3.so.0 '+' '.join(libraries), 1)
        graph = graph.replace('  libs = -lSDL3 -lm -lpthread -ldl',
            '  libs = -Wl,--start-group '+' '.join(libraries)+' -Wl,--end-group -lSDL3 -lm -lpthread -ldl', 1)
        graph += '\n'+extra_graph
        (output/'plan.json').write_text(json.dumps(plan, indent=2)+'\n')
        (output/'build.ninja').write_text(graph)
        commands = json.loads(subprocess.check_output(
            ['ninja', '-f', str(output/'build.ninja'), '-t', 'compdb', 'linux_cc'], cwd=source, text=True))
        tracked = subprocess.check_output(
            ['ninja', '-f', str(output/'build.ninja'), '-t', 'query', 'build/gamepkg-compiler.stamp'], cwd=source, text=True)
        tracked = {line.strip() for line in tracked.split('  outputs:\n', 1)[1].splitlines()}
        assert commands and all(command['output'] in tracked for command in commands)
        bridge = next(x for x in commands if x['file'].endswith('gamepkg_bink_bridge.c'))
        assert not any(x in bridge['command'] for x in ['-malign-double', '-fshort-wchar', '-freg-struct-return', '-include'])
        assert len([x for x in commands if '/ffmpeg/' in x['file']]) == 229
        print('Exported verified movie decoder and native-ABI bridge; compiler graph checked')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--ffmpeg-source', type=Path, required=True)
    parser.add_argument('--ffmpeg-archive', type=Path, required=True)
    parser.add_argument('--ffmpeg-build-log', type=Path, required=True)
    args = parser.parse_args()
    extend(*(value.resolve() for value in [args.source, args.output, args.ffmpeg_source,
                                          args.ffmpeg_archive, args.ffmpeg_build_log]))


if __name__ == '__main__':
    main()
