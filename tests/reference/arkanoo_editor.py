"""Maintainer oracle: compile the complete unchanged pinned upstream editor module."""
from pathlib import Path
import hashlib,json,subprocess,sys,tempfile
root=Path(sys.argv[1]).resolve();source=root/'src/editor.rs';entities=(root/'src/entities.rs').read_text()
constants=entities[entities.index('pub const WINDOW_WIDTH'):entities.index('#[derive(Clone, Copy, PartialEq)]')]
types=entities[entities.index('#[derive(Clone, Copy, PartialEq)]'):entities.index('pub struct Paddle')]
block=entities[entities.index('#[derive(Clone)]'):entities.index('pub struct Bonus')]
colors=entities[entities.index('pub const BLOCK_COLORS'):entities.index('pub fn create_blocks')]
header='''#![allow(dead_code,unused_variables)]
extern crate self as sdl2;
pub mod rect {pub struct Rect{x:i32,y:i32,w:u32,h:u32} impl Rect {pub fn new(x:i32,y:i32,w:u32,h:u32)->Self{Self{x,y,w,h}}pub fn contains_point(&self,p:(i32,i32))->bool{p.0>=self.x&&p.1>=self.y&&p.0<self.x+self.w as i32&&p.1<self.y+self.h as i32}}}
mod menu {pub struct Button;impl Button{pub fn new(_:i32,_:i32,_:u32,_:u32,_:&str)->Self{Self}pub fn update_hover(&mut self,_:i32,_:i32){}}}
'''
body=header+'mod entities{use crate::rect::Rect;'+constants+types+block+colors+'}\n#[path='+json.dumps(str(source))+']mod editor;\nuse editor::*;\n'
body+='''fn snapshot(e:&LevelEditor){let mut cells=[255u8;200];let mut health=[0u8;200];for b in &e.blocks{let i=((b.y-80)/20) as usize*20+((b.x-40)/60) as usize;cells[i]=match b.block_type{entities::BlockType::Normal=>entities::BLOCK_COLORS.iter().position(|c|c.r==b.color.r&&c.g==b.color.g&&c.b==b.color.b).unwrap() as u8,entities::BlockType::Ice=>6,entities::BlockType::Explosive=>7,entities::BlockType::Undestroyable=>8};health[i]=b.health;}println!("{:?}",cells);println!("{:?}",health);}
fn main(){let mut e=LevelEditor::new();
'''
# An ordered sequence covers boundaries, duplicate painting, nine choices and drag priority.
operations=[['add',39,80],['add',40,79],['add',1240,80],['add',40,280]]
for i in range(9):operations += [['select',i,0],['add',40+i*60,80],['add',41+i*60,81]]
operations += [['select',3,0],['add',40,80],['left',100,100],['move',160,100],['right',220,100],['move',280,100],['stop',0,0],['right',40,80],['move',100,80],['stop',0,0],['add',1239,279]]
for op,x,y in operations:
    statements={'select':f'e.selected_color_index={x};','add':f'e.add_block_at({x},{y});','left':f'e.start_drag_left({x},{y});','right':f'e.start_drag_right({x},{y});','move':f'e.update_drag({x},{y});','stop':'e.stop_drag();'}
    body+=statements[op]+'snapshot(&e);\n'
body+='''e.pattern_name="example".into();e.save_pattern().unwrap();println!("{:?}",std::fs::read_to_string("patterns/example.txt").unwrap());e.load_pattern("example").unwrap();snapshot(&e);
e.request_clear();println!("{}",e.confirm_clear);e.cancel_clear();println!("{}",e.confirm_clear);e.request_clear();e.clear();snapshot(&e);
e.pattern_name="café😀".into();e.pattern_name_editing=true;e.handle_backspace();println!("{:?}",e.pattern_name);e.handle_backspace();println!("{:?}",e.pattern_name);
for _ in 0..8{e.next_background();println!("{}",e.current_background);}for _ in 0..8{e.prev_background();println!("{}",e.current_background);}
'''
ascii_cases=['# ignored\n\n012345678IEU*x\n**6\n', '\n'.join(['8'*25]*12), '  #not-a-comment\n0\n  \nI E U\n']
for text in ascii_cases:
    body+='std::fs::write("pattern.txt",'+json.dumps(text)+').unwrap();let p=PatternData::load_from_file("pattern.txt").unwrap();let cells:Vec<u8>=p.grid.iter().flatten().map(|c|if c.color_index==255{255}else{match c.block_type{entities::BlockType::Normal=>c.color_index,entities::BlockType::Ice=>6,entities::BlockType::Explosive=>7,entities::BlockType::Undestroyable=>8}}).collect();println!("{:?}",cells);\n'
body+='}\n'
with tempfile.TemporaryDirectory(prefix='arkanoo-editor-reference-') as temporary:
    path=Path(temporary);(path/'main.rs').write_text(body)
    subprocess.run(['rustc','--edition=2021',str(path/'main.rs'),'-o',str(path/'reference')],check=True)
    lines=iter(subprocess.check_output([str(path/'reference')],cwd=path,text=True).splitlines())
    steps=[]
    for op in operations:steps.append(dict(operation=op,cells=json.loads(next(lines)),health=json.loads(next(lines))))
    ascii_saved=json.loads(next(lines));loaded=dict(cells=json.loads(next(lines)),health=json.loads(next(lines)))
    confirmations=[json.loads(next(lines)),json.loads(next(lines))];cleared=[json.loads(next(lines)),json.loads(next(lines))]
    backspaces=[json.loads(next(lines)),json.loads(next(lines))];backgrounds=[int(next(lines)) for _ in range(16)]
    imports=[dict(source=text,cells=json.loads(next(lines))) for text in ascii_cases]
    assert list(lines)==[]
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',editor_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),steps=steps,ascii_saved=ascii_saved,loaded=loaded,confirmations=confirmations,cleared=cleared,backspaces=backspaces,backgrounds=backgrounds,imports=imports),indent=2,ensure_ascii=False))
