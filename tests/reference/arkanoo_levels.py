"""Maintainer oracle: unchanged original create_blocks with pinned rand 0.8.5."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess
import tempfile
import tomllib

parser=argparse.ArgumentParser();parser.add_argument('upstream');parser.add_argument('--rand-source',required=True,help='Extracted rand 0.8.5 source directory');parser.add_argument('--custom',action='store_true',help='Verify a three-pattern custom pool, including special/empty layouts');args=parser.parse_args()
rand_source=Path(args.rand_source).resolve()
assert tomllib.loads((rand_source/'Cargo.toml').read_text())['package']['version']=='0.8.5', 'reference requires exactly rand 0.8.5'
source=Path(args.upstream)/'src/entities.rs';text=source.read_text()
def method(source,name):
    begin=source.index('pub fn '+name+'(');opening=source.index('{',begin);depth=1;end=opening+1
    while depth:
        depth+=(source[end]=='{')-(source[end]=='}');end+=1
    return source[begin:end]
constants=text[text.index('pub const WINDOW_WIDTH'):text.index('#[derive(Clone, Copy, PartialEq)]')]
types=text[text.index('#[derive(Clone, Copy, PartialEq)]'):text.index('pub struct Paddle')]
block=text[text.index('#[derive(Clone)]'):text.index('pub struct Bonus')]
colors=text[text.index('pub const BLOCK_COLORS'):text.index('pub fn create_blocks')]
generator=method(text,'create_blocks')
# Diagnostic copy only: expected final layouts always use the unchanged function.
first_attempt=generator.replace('pub fn create_blocks(', 'fn first_attempt(').replace('if level <= 9 || blocks.len() >= MIN_BLOCKS || attempt >= MAX_RETRIES {','if true {')
levels=list(range(1,101))+[500,999,1000,65535,2147483648,4294967295]
seeds=[0,1,10,1234,4294967295,18446744073709551615]
header='''#![allow(dead_code,unused_variables,unreachable_code)]
use rand::{Rng,RngCore,SeedableRng};use rand::rngs::StdRng;
pub struct Rect;impl Rect{fn new(_:i32,_:i32,_:u32,_:u32)->Self{Self}}
mod once_cell{pub mod sync{pub type Lazy<T>=std::sync::LazyLock<T>;}}
mod editor{pub struct PatternData;pub fn load_all_patterns(_: &str)->Vec<PatternData>{Vec::new()}pub fn create_blocks_from_pattern(_: &PatternData)->Vec<super::Block>{panic!("no custom patterns in procedural oracle")}}
'''
if args.custom:
    editor_text=(Path(args.upstream)/'src/editor.rs').read_text()
    pattern_types=editor_text[editor_text.index('#[derive(Clone)]'):editor_text.index('impl PatternData')]
    converter=method(editor_text,'create_blocks_from_pattern')
    custom_module='mod editor{use super::*;'+pattern_types+"""
    pub fn load_all_patterns(_: &str)->Vec<PatternData>{let mut result=Vec::new();for i in 0..3{let mut p=PatternData{name:format!("custom{}",i),grid:[[PatternCell{color_index:255,block_type:BlockType::Normal};20];10]};if i==0{for c in 0..9{p.grid[0][c]=PatternCell{color_index:if c<6{c as u8}else{0},block_type:match c{6=>BlockType::Ice,7=>BlockType::Explosive,8=>BlockType::Undestroyable,_=>BlockType::Normal}};}}if i==1{for r in 0..10{p.grid[r][r]=PatternCell{color_index:(r%6) as u8,block_type:BlockType::Normal};}}result.push(p);}result}
    """+converter+'}\n'
    old='mod editor{pub struct PatternData;pub fn load_all_patterns(_: &str)->Vec<PatternData>{Vec::new()}pub fn create_blocks_from_pattern(_: &PatternData)->Vec<super::Block>{panic!("no custom patterns in procedural oracle")}}'
    assert old in header;header=header.replace(old,custom_module)
body=header+constants+types+block+colors+generator+'\n'+first_attempt+'\nfn main(){assert_eq!(usize::BITS,64,"canonical original layouts use the pinned 64-bit build");\n'
for seed in seeds:
    body+=f'let mut r=StdRng::seed_from_u64({seed});println!("{{:?}}",(0..128).map(|_|r.next_u32()).collect::<Vec<_>>());\n'
    body+=f'let mut r=StdRng::seed_from_u64({seed});println!("{{:?}}",(0..64).map(|i|{{let (l,h)=[(0u64,12u64),(2,5),(0,3),(2,6)][i%4];r.gen_range(l..h)}}).collect::<Vec<_>>());\n'
    body+=f'let mut r=StdRng::seed_from_u64({seed});println!("{{:?}}",(0..64).map(|i|{{let (l,h)=[(0.6f32,0.8f32),(-0.3,0.3),(1.5,3.0)][i%3];r.gen_range(l..h).to_bits()}}).collect::<Vec<_>>());\n'
    body+=f'let mut r=StdRng::seed_from_u64({seed});println!("{{:?}}",(0..64).map(|_|r.gen::<f32>().to_bits()).collect::<Vec<_>>());\n'
    body+=f'let mut r=StdRng::seed_from_u64({seed});println!("{{:?}}",(0..64).map(|_|u8::from(r.gen_bool(0.6))).collect::<Vec<_>>());\n'
if args.custom:
    body+='for p in editor::load_all_patterns("unused"){let b=editor::create_blocks_from_pattern(&p);let mut cells=[255u8;200];for v in b{let i=((v.y-80)/20) as usize*20+((v.x-40)/60) as usize;cells[i]=match v.block_type{BlockType::Normal=>BLOCK_COLORS.iter().position(|c|c.r==v.color.r&&c.g==v.color.g&&c.b==v.color.b).unwrap() as u8,BlockType::Ice=>6,BlockType::Explosive=>7,BlockType::Undestroyable=>8};}println!("{:?}",cells); }\n'
    for level in levels:
        body+=f'let b=create_blocks({level});let mut cells=[255u8;200];for v in b{{let i=((v.y-80)/20) as usize*20+((v.x-40)/60) as usize;cells[i]=match v.block_type{{BlockType::Normal=>BLOCK_COLORS.iter().position(|c|c.r==v.color.r&&c.g==v.color.g&&c.b==v.color.b).unwrap() as u8,BlockType::Ice=>6,BlockType::Explosive=>7,BlockType::Undestroyable=>8}};}}println!("{{:?}}",cells);\n'
else:
    for level in levels:
        body+=f'let b=create_blocks({level});let count=first_attempt({level}).len();let mut cells=[0u8;200];for block in &b{{assert!(matches!(block.block_type,BlockType::Normal));let i=((block.y-BLOCK_OFFSET_Y)/BLOCK_HEIGHT) as usize*20+((block.x-40)/BLOCK_WIDTH) as usize;cells[i]=1;}}println!("{{}}:{{}}",count,cells.iter().map(|v|if *v==1{{\'1\'}}else{{\'0\'}}).collect::<String>());\n'
body+='}\n'
with tempfile.TemporaryDirectory(prefix='arkanoo-level-reference-') as temporary:
    directory=Path(temporary);(directory/'src').mkdir();(directory/'src/main.rs').write_text(body)
    manifest='[package]\nname="arkanoo-level-reference"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nrand={path='+json.dumps(str(rand_source))+'}\nrand_chacha="=0.3.1"\nrand_core="=0.6.4"\n'
    (directory/'Cargo.toml').write_text(manifest)
    env=dict(os.environ,CARGO_TARGET_DIR='/tmp/gamepkg-arkanoo-level-reference-target')
    output=subprocess.check_output(['cargo','run','--release','--offline','--quiet','--manifest-path',str(directory/'Cargo.toml')],env=env,text=True)
    lock=tomllib.loads((directory/'Cargo.lock').read_text())
    versions={package['name']:package['version'] for package in lock['package'] if package['name'] in ('rand','rand_chacha','rand_core')}
    assert versions=={'rand':'0.8.5','rand_chacha':'0.3.1','rand_core':'0.6.4'}
    lines=iter(output.splitlines());rng=[]
    for seed in seeds:
        rng.append(dict(seed=seed,words=json.loads(next(lines)),ranges=json.loads(next(lines)),floats=json.loads(next(lines)),units=json.loads(next(lines)),booleans=json.loads(next(lines))))
    fixtures=[]
    if args.custom:
        patterns=[json.loads(next(lines)) for _ in range(3)]
        for level in levels:fixtures.append(dict(level=level,cells=json.loads(next(lines))))
        print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rand_versions=versions,canonical_pointer_bits=64,generator_sha256=hashlib.sha256(generator.encode()).hexdigest(),converter_sha256=hashlib.sha256(converter.encode()).hexdigest(),patterns=patterns,layouts=fixtures),indent=2))
    else:
        for level in levels:
            count,cells=next(lines).split(':');fixtures.append(dict(level=level,first_attempt_count=int(count),cells=cells))
        print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rand_versions=versions,rand='0.8.5',canonical_pointer_bits=64,generator_sha256=hashlib.sha256(generator.encode()).hexdigest(),rng=rng,layouts=fixtures),indent=2))
