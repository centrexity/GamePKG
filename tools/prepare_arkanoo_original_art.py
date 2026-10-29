"""Maintainer-only full-resolution artwork candidate; never rewrites the active port.

Players build the resulting Rust recipe without Python or image conversion tools.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tomllib
from PIL import Image


def prepare(originals: Path, destination: Path, base_port: Path | None = None) -> None:
    project = Path(__file__).resolve().parents[1]
    source = base_port or project / 'ports/arkanoo'
    base = tomllib.loads((source / 'manifest.toml').read_text())
    assert base['version'] == '0.1.74' and base['limits'].get('render_profile') is None, 'Require the unchanged 0.1.74 recipe as --base-port'
    assert not destination.exists(), 'Candidate destination must be new'
    # The independent original-image oracle pins these same six source files.
    oracle = json.loads((project / 'tests/fixtures/arkanoo-image-backgrounds.json').read_text())
    assert oracle['revision'] == '9d613e78c9ed5657ae7fda9206cd4a82a893b7c8'
    originals = originals.resolve()
    images = []
    for index, case in enumerate(oracle['cases'], 1):
        path = originals / f'background{index}.png'
        assert path.is_file() and not path.is_symlink(), path
        assert hashlib.sha256(path.read_bytes()).hexdigest() == case['original_sha256'], path
        with Image.open(path) as image:
            rgba = image.convert('RGBA')
        assert rgba.size == (2816, 1536) and rgba.getchannel('A').getextrema() == (255, 255)
        images.append((path, rgba))
    assert len(images) == 6
    shutil.copytree(source, destination)
    assets = destination / 'assets'
    for pattern in ['background*.png', 'menu-background*.png']:
        for path in assets.glob(pattern):
            path.unlink()
    provenance = json.loads((assets / 'ASSET_PROVENANCE.json').read_text())
    retained = [row for row in provenance['image_optimization']['images']
                if not row['asset'].startswith(('background', 'menu-background'))]
    rows = []
    sources = []
    for number, (path, image) in enumerate(images, 1):
        sources.append({'source': path.name, 'source_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                        'size': list(image.size), 'rgba_sha256': hashlib.sha256(image.tobytes()).hexdigest()})
        reconstructed = Image.new('RGBA', image.size)
        for index in range(24):
            x, y = index % 6 * 510, index // 6 * 510
            width, height = min(510, 2816-x), min(510, 1536-y)
            left, top = max(0, x-1), max(0, y-1)
            right, bottom = min(2816, x+width+1), min(1536, y+height+1)
            tile = image.crop((left, top, right, bottom))
            name = f'background{number}-{index}.png'
            tile.convert('RGB').save(assets / name, optimize=True)
            reconstructed.paste(tile.crop((x-left, y-top, x-left+width, y-top+height)), (x, y))
            rows.append({'asset': name, 'size': list(tile.size), 'core': [x, y, width, height],
                         'source_rectangle': [left, top, right-left, bottom-top],
                         'rgba_sha256': hashlib.sha256(tile.tobytes()).hexdigest(),
                         'encoded_sha256': hashlib.sha256((assets / name).read_bytes()).hexdigest()})
        assert reconstructed.tobytes() == image.tobytes(), 'Lossless reconstruction failed'
    provenance['original_backgrounds'] = {
        'method': 'Lossless source-resolution RGB tiles, 510-pixel cores and one-pixel filtering halos; no resampling. Menu reuses background1.',
        'sources': sources, 'tiles': rows,
    }
    provenance['image_optimization']['images'] = sorted(retained + rows, key=lambda row: row['asset'])
    (assets / 'ASSET_PROVENANCE.json').write_text(json.dumps(provenance, indent=2) + '\n')

    render = destination / 'source/src/render.rs'
    text = render.read_text()
    begin = text.index('pub fn draw_image_background(')
    end = text.index('\npub fn ', begin+1)
    # Preserve all other rendering code and the exact darkness overlay order.
    cases = '\n'.join(f'        {number} => [' + ', '.join(
        f'"background{number}-{index}.png"' for index in range(24)) + '],'
        for number in range(1, 7))
    replacement = '''pub fn draw_image_background(level: u32, darkness: u8) -> bool {
    let paths = match level {
CASES
        _ => return false,
    };
    let mut accepted = draw_original_tiles(paths);
    if darkness > 0 && !gamepkg::draw_rect(0.0, 60.0, 640.0, 360.0, u32::from(darkness)) {
        accepted = false;
    }
    accepted
}
'''.replace('CASES', cases)
    text = text[:begin] + replacement + text[end:]
    begin = text.index('pub fn draw_menu_background()')
    end = text.index('pub fn draw_menu(', begin)
    splash_helper = text[text.index('fn draw_world_tiles(', begin):end]
    replacement = '''pub fn draw_menu_background() -> bool {
    draw_image_background(1, 150)
}
fn draw_original_tiles(paths: [&str; 24]) -> bool {
    let mut accepted = true;
    let scale_x = 640.0 / 2816.0;
    let scale_y = 360.0 / 1536.0;
    for index in 0..24 {
        let core_x = (index % 6) * 510usize;
        let core_y = (index / 6) * 510usize;
        let core_w = (2816 - core_x).min(510);
        let core_h = (1536 - core_y).min(510);
        let left = core_x.saturating_sub(1);
        let top = core_y.saturating_sub(1);
        let right = (core_x + core_w + 1).min(2816);
        let bottom = (core_y + core_h + 1).min(1536);
        let x = core_x as f32 * scale_x;
        let y = 60.0 + core_y as f32 * scale_y;
        let x1 = ((core_x + core_w) as f32 * scale_x).min(640.0);
        let y1 = 60.0 + (core_y + core_h) as f32 * scale_y;
        if !gamepkg::draw_clip(x, y, x1-x, y1-y) { accepted = false; }
        if !gamepkg::draw_asset_image_linear(paths[index], left as f32 * scale_x,
            60.0 + top as f32 * scale_y, (right-left) as f32 * scale_x,
            (bottom-top) as f32 * scale_y) { accepted = false; }
        if !gamepkg::draw_clip_reset() { accepted = false; }
    }
    accepted
}
'''
    render.write_text(text[:begin] + replacement + splash_helper + text[end:])
    manifest = destination / 'manifest.toml'
    text = manifest.read_text().replace('version = "0.1.74"', 'version = "0.1.75"')
    text = text.replace('[limits]', '[limits]\nrender_profile = "large-images-v1"')
    text = text.replace('recommended_memory_bytes = 16777216', 'recommended_memory_bytes = 268435456')
    text = text.replace('hard_memory_bytes = 16777216', 'hard_memory_bytes = 268435456')
    storage = sum(path.stat().st_size for path in destination.rglob('*') if path.is_file())
    text = text.replace('recommended_storage_bytes = 58720256', f'recommended_storage_bytes = {((storage+1048575)//1048576+8)*1048576}')
    manifest.write_text(text)
    limits = tomllib.loads(text)['limits']
    assert limits['render_profile'] == 'large-images-v1'
    assert (limits['hard_memory_bytes'], limits['effects_memory_bytes'], limits['music_memory_bytes']) == (268435456, 3145728, 7340032)
    for path in (source / 'source/src').glob('*.rs'):
        if path.name != 'render.rs':
            assert path.read_bytes() == (destination / 'source/src' / path.name).read_bytes(), path
    credits = destination / 'CREDITS.md'
    text = credits.read_text().replace('canonical 0.1.74 recipe', 'canonical 0.1.75 candidate recipe').replace('all 76 losslessly optimized image hashes', 'all 178 lossless image hashes')
    text += '\n\nSource-resolution artwork candidate (0.1.75, unreleased):\nThe six pinned 2816×1536 backgrounds retain every original pixel in 144 lossless RGB tiles with filtering halos. The menu reuses background1; the 34 other images, original effects, all nine music tracks, game logic and controls are unchanged. The signed large-images-v1 profile requires a 256 MiB hard/recommended game-memory limit, retains the original 3/7 MiB effect/music shares and separately bounds host artwork buffers. Previous 16 MiB/8 MiB legacy rendering is frozen as a host test fixture. This candidate addresses original-photo sampling; broader fidelity and platform qualifications remain separate.\n'
    credits.write_text(text)
    pngs = list(assets.glob('*.png'))
    assert len(pngs) == 178
    print('Prepared complete original-artwork candidate:', destination)
    print('Six exact source reconstructions; 144 source-resolution tiles, 34 retained images; all effects/music unchanged.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('original_assets', type=Path)
    parser.add_argument('destination', type=Path)
    parser.add_argument('--base-port', type=Path, help='Pinned 0.1.74 recipe; defaults to the active port before migration')
    args = parser.parse_args()
    prepare(args.original_assets, args.destination, args.base_port)
