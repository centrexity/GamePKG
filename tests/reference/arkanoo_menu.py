"""Maintainer oracle: compile the complete unchanged pinned original menu module."""
from pathlib import Path
import hashlib, json, os, subprocess, sys, tempfile, tomllib
root=Path(sys.argv[1]).resolve();source=root/'src/menu.rs'
assert hashlib.sha256(source.read_bytes()).hexdigest()=='63bcc800d4d7b50eaf89a87c4c2e1c074d76b2cf578c777ce38edb0806903f28','reference requires the pinned original menu module'
header='''#![allow(dead_code)]
extern crate self as sdl2;
pub mod rect {
#[derive(Clone)]pub struct Rect{x:i32,y:i32,w:u32,h:u32}
impl Rect{pub fn new(x:i32,y:i32,w:u32,h:u32)->Self{Self{x,y,w,h}}pub fn x(&self)->i32{self.x}pub fn y(&self)->i32{self.y}pub fn width(&self)->u32{self.w}pub fn height(&self)->u32{self.h}pub fn contains_point(&self,p:(i32,i32))->bool{p.0>=self.x&&p.0<self.x+self.w as i32&&p.1>=self.y&&p.1<self.y+self.h as i32}}
}
'''
body=header+'#[path='+json.dumps(str(source))+']mod original;use original::*;\n'
body+='''fn code(action:MenuAction)->u32{match action{MenuAction::None=>0,MenuAction::NewGame|MenuAction::Resume=>1,MenuAction::Restart=>2,MenuAction::ToggleGravity=>3,MenuAction::EnterLevelEditor=>4,MenuAction::OpenSettings=>5,MenuAction::Quit=>6,MenuAction::ToggleFullscreen=>7,MenuAction::CloseSettings=>8,MenuAction::OpenGithub=>9,MenuAction::ToggleSFX=>10,_=>99}}
fn main(){let mut m=Menu::new(1280,720);
'''
cases=[]
for settings in [False,True]:
    body+='m.state=MenuState::'+('Settings' if settings else 'Main')+';\n'
    entries=[(7,540,400,200,40),(8,540,760,200,40),(10,540,310,200,40)] if settings else [(i+1,540,235+i*50,200,40) for i in range(6)]
    for action,x,y,w,h in entries:
        for px,py in [(x,y),(x-1,y),(x+w-1,y+h-1),(x+w,y),(x,y+h),(x+w/2,y+h/2),(x-.25,y),(x,y-.25),(x+w-.25,y+h-.25)]:
            cases.append(dict(settings=settings,point=[px*.5,py*.5+60],button=action))
            body+=f'println!("{{}}",code(handle_menu_click(&m,({float(px)}f32) as i32,({float(py)}f32) as i32)));\n'
# Mirror the original event loop: button events change mouse_down; only motion updates sliders.
slider_events=[[1,320,245,0],[2,320,245,1],[1,295,245,1],[1,390,245,1],[1,250,245,1],[3,250,245,1],[1,320,245,0],[2,260,245,1],[1,280,245,1],[1,345.25,245,1],[3,345.25,245,1],[1,270,245,0],[2,320,245,2],[1,350,245,0],[2,320,245,1],[3,320,245,4],[1,330,245,1],[3,330,245,1],[1,350,245,0],[2,320,245,1],[1,390,245,1],[1,320,245,1],[1,390,245,1],[3,390,245,1],[1,350,245,0]]
body+='m.state=MenuState::Settings;let mut down=false;\n'
for kind,x,y,button in slider_events:
    if kind==2 and button==1:body+='down=true;'
    if kind==3 and button==1:body+='down=false;'
    if kind==1:body+=f'm.update_slider(({x*2}f32) as i32,({(y-60)*2}f32) as i32,down);'
    body+='println!("{}",m.sfx_slider.get_value());\n'
body+='}'

with tempfile.TemporaryDirectory(prefix='gamepkg-arkanoo-menu-reference-') as temporary:
    folder=Path(temporary);(folder/'main.rs').write_text(body)
    env=dict(os.environ,CARGO_PKG_VERSION=tomllib.loads((root/'Cargo.toml').read_text())['package']['version'])
    subprocess.run(['rustc','--edition=2021',folder/'main.rs','-o',folder/'oracle'],env=env,check=True)
    results=subprocess.check_output([folder/'oracle'],text=True).splitlines();assert len(results)==len(cases)+len(slider_events)
    for case,result in zip(cases,results[:len(cases)]):case['action']=int(result)
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',menu_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),cases=cases,slider=[dict(event=event,volume=int(value)) for event,value in zip(slider_events,results[len(cases):])],remaining=['Original splash/menu transitions','Original music settings and recorded effects','Resolution/VSync controls and confirm/revert','Project-homepage button','Original fonts/background artwork']),indent=2))
