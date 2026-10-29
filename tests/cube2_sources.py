"""Pinned Cube 2 source integrity and adaptation anchors (maintainer test)."""
import importlib.util
from pathlib import Path
import shutil
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('cube2_build', PROJECT / 'tools/build_cube2_engine.py')
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)
lock = build.verify()
assert lock['revision'] == 6889
assert not any(Path(name).suffix in {'.lib', '.a', '.dll', '.exe', '.o'} for name in lock['files'])
with tempfile.TemporaryDirectory(prefix='cube2-integrity-') as temporary:
    source = Path(temporary) / 'source'
    shutil.copytree(build.SOURCE, source)
    def rejected():
        try:
            build.verify(source)
        except ValueError:
            return
        raise AssertionError('Modified source passed integrity verification')
    path = source / 'engine/main.cpp'
    original = path.read_bytes()
    path.write_bytes(original + b'\n// tampered\n')
    rejected()
    path.write_bytes(original)
    extra = source / 'engine/unexpected.cpp'
    extra.write_text('int injected;')
    rejected()
    extra.unlink()
    path.unlink()
    rejected()
    path.symlink_to(build.SOURCE / 'engine/main.cpp')
    rejected()
assert build.replace_once('before', 'before', 'after') == 'after'
for source in ['missing', 'before before']:
    try:
        build.replace_once(source, 'before', 'after')
    except ValueError:
        continue
    raise AssertionError('Changed adaptation anchor accepted')
print('Cube 2 pinned sources: hashes, extra/missing files, symlinks and changed adaptation anchors rejected.')

with tempfile.TemporaryDirectory(prefix='cube2-socket-headers-') as temporary:
    stage=Path(temporary)
    shutil.copytree(build.SOURCE/'enet/include',stage/'enet/include')
    header=stage/'enet/include/enet/unix.h';original=header.read_text()
    build.adapt_socket_sets(stage)
    assert 'typedef fd_set ENetSocketSet;' not in header.read_text()
    assert (stage/'enet/include/enet/gamepkg_socket_set.h').read_bytes()==(PROJECT/'engines/cube2/adapters/socket_set.h').read_bytes()
    try:build.adapt_socket_sets(stage)
    except ValueError:pass
    else:raise AssertionError('Repeated socket adaptation accepted')
    header.write_text(original.replace('typedef fd_set ENetSocketSet;','typedef int ENetSocketSet;'))
    try:build.adapt_socket_sets(stage)
    except ValueError:pass
    else:raise AssertionError('Changed socket-set anchor accepted')
build.verify()
print('Opaque socket-set overlay: staged only; repeated/tampered anchors rejected; original source hashes unchanged.')

with tempfile.TemporaryDirectory(prefix='cube2-movie-audio-') as temporary:
    stage=Path(temporary);(stage/'engine').mkdir()
    movie=stage/'engine/movie.cpp'
    original=(build.SOURCE/'engine/movie.cpp').read_text();movie.write_text(original)
    build.adapt_movie_audio(stage)
    assert 'Mix_SetPostMix(' not in movie.read_text()
    assert movie.read_text().count('gamepkg_audio_postmix(')==3
    try:build.adapt_movie_audio(stage)
    except ValueError:pass
    else:raise AssertionError('Repeated movie audio adaptation accepted')
    movie.write_text(original.replace('Mix_SetPostMix(soundencoder, NULL);','Mix_SetPostMix(NULL, NULL);'))
    try:build.adapt_movie_audio(stage)
    except ValueError:pass
    else:raise AssertionError('Changed movie audio adaptation anchor accepted')
build.verify()
print('Movie audio overlay: staged only; callback anchors checked; pinned original hashes unchanged.')

with tempfile.TemporaryDirectory(prefix='cube2-movie-limit-') as temporary:
    stage=Path(temporary);(stage/'engine').mkdir()
    movie=stage/'engine/movie.cpp'
    original=(build.SOURCE/'engine/movie.cpp').read_text();movie.write_text(original)
    build.adapt_movie_audio(stage)
    build.adapt_movie_limits(stage)
    assert (stage/'engine/gamepkg_movie_limit.h').read_bytes()==(PROJECT/'engines/cube2/adapters/movie_limit.h').read_bytes()
    assert (stage/'engine/gamepkg_movie_io.h').read_bytes()==(PROJECT/'engines/cube2/adapters/movie_io.h').read_bytes()
    assert movie.read_text().count('gamepkg_budget.allow(')==2
    assert 'return writechunk("01wb", data, framesize);' in movie.read_text()
    assert 'return writechunk("00dc", format == VID_YUV420 ? pixels : yuv, framesize);' in movie.read_text()
    assert 'seg.indexframes = indexframes;' in movie.read_text()
    assert 'f->putlil<uint>(segments[0].videoframes);' not in movie.read_text()
    assert 'f->putlil<uint>(segments[0].soundsamples);' in movie.read_text()
    assert 'f->putlil<uint>(seg.soundsamples);' in movie.read_text()
    try:build.adapt_movie_limits(stage)
    except ValueError:pass
    else:raise AssertionError('Repeated movie limit adaptation accepted')
    movie.write_text(original.replace('vector<aviindexentry> index;', 'vector<aviindexentry> changed;'))
    try:build.adapt_movie_limits(stage)
    except ValueError:pass
    else:raise AssertionError('Changed movie limit adaptation anchor accepted')
build.verify()
print('Movie limit overlay: staged only; repeated/tampered anchors rejected; pinned sources unchanged.')

with tempfile.TemporaryDirectory(prefix='cube2-zip-bounds-') as temporary:
    stage=Path(temporary);(stage/'shared').mkdir()
    source=stage/'shared/zip.cpp'
    original=(build.SOURCE/'shared/zip.cpp').read_text();source.write_text(original)
    build.adapt_zip_bounds(stage)
    adapted=source.read_text()
    assert 'lilswap(*(uint *)src)' not in adapted and 'lilswap(*(ushort *)src)' not in adapted
    assert '*(uint *)search' not in adapted
    assert 'gamepkg_zip_find_end(buf' in adapted
    assert adapted.index('gamepkg_zip_directory_valid(ftell(f)') < adapted.index('new (false) uchar[size]')
    assert (stage/'shared/gamepkg_zip_bounds.h').read_bytes()==(PROJECT/'engines/cube2/adapters/zip_bounds.h').read_bytes()
    try:build.adapt_zip_bounds(stage)
    except ValueError:pass
    else:raise AssertionError('Repeated ZIP bounds adaptation accepted')
    source.write_text(original.replace('uchar *buf = new (false) uchar[size], *src = buf;', 'uchar *buf = NULL, *src = buf;'))
    try:build.adapt_zip_bounds(stage)
    except ValueError:pass
    else:raise AssertionError('Changed ZIP bounds adaptation anchor accepted')
build.verify()
print('ZIP directory bounds overlay: before-allocation guard, repeated/tampered anchors and pinned source integrity passed.')
