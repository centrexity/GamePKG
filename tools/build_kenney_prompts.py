"""Maintainer-only vector rendering. Players compile the generated C data directly."""
from pathlib import Path
import argparse,concurrent.futures,hashlib,json,subprocess,tempfile
from PIL import Image,ImageDraw
import numpy as np
ROOT=Path(__file__).resolve().parents[1]
ART=ROOT/'engines/takedawn/media-bridge/input-prompts'
CELLS=[(0,0,30,30),(30,0,30,30),(60,0,30,30),(90,0,30,30),(1,37,28,19),(31,37,28,19),(62,29,18,18),(62,46,18,18),(80,29,48,35)]
SWAPPED={3,7,10,11,12,13,14}

def source(platform,name):return f'{platform}/Vector/{name}.svg'
def family(platform,prefix,faces,shoulders,start,dpad):
    # Texture cells are B A X Y, white, black, left/right trigger, Start.
    return [source(platform,prefix+x) for x in faces+shoulders+[start]]+[source(platform,x) for x in dpad]

def choices():
    xbox=family('Xbox Series','xbox_', ['button_color_b','button_color_a','button_color_x','button_color_y'],['lb','rb','lt','rt'],'button_menu',['xbox_dpad_down','xbox_dpad_right'])
    original=list(xbox);original[4]=source('Generic','generic_button_circle_fill');original[5]=source('Generic','generic_button_circle');original[8]=source('Xbox Series','xbox_button_start')
    nintendo=family('Nintendo Switch','switch_', ['button_a','button_b','button_y','button_x'],['button_l','button_r','button_zl','button_zr'],'button_plus',['switch_dpad_down','switch_dpad_right'])
    steam=family('Steam Deck','steamdeck_', ['button_b','button_a','button_x','button_y'],['button_l1','button_r1','button_l2','button_r2'],'button_options',['steamdeck_dpad_down','steamdeck_dpad_right'])
    positional=list(steam)
    for i,d in enumerate(['right','down','left','up']):positional[i]=source('Nintendo Switch','switch_buttons_'+d)
    keyboard=[source('Keyboard & Mouse','keyboard_'+x) for x in ['backspace_icon','space','shift','ctrl','q','e','s','w','enter','arrow_down','arrow_right']]
    def swap(value):
        value=list(value);value[0],value[1]=value[1],value[0];value[2],value[3]=value[3],value[2];return value
    return [original,None,nintendo,swap(nintendo),steam,xbox,positional,swap(positional),None,keyboard,swap(original),None,None,swap(steam),swap(xbox)]

def compose(paths,images,factor,original=False):
    sheet=Image.new('RGBA',(128*factor,64*factor));draw=ImageDraw.Draw(sheet)
    for i,(x,y,w,h) in enumerate(CELLS):
        x,y,w,h=[n*factor for n in (x,y,w,h)];pad=max(1,factor)
        box=(x+pad,y+pad,x+w-pad-1,y+h-pad-1)
        if i<4:draw.ellipse(box,fill=(15,23,35,255))
        else:draw.rounded_rectangle(box,radius=max(2,3*factor),fill=(15,23,35,245))
        im=images[paths[i]].copy();im.thumbnail((w-4*factor,h-4*factor),Image.Resampling.LANCZOS)
        if original and i in (4,5):
            # The original Xbox's physical white/black buttons keep their colors.
            a=im.getchannel('A');im=Image.new('RGBA',im.size,(245,245,245,0) if i==4 else (45,45,48,0));im.putalpha(a)
        sheet.alpha_composite(im,(x+(w-im.width)//2,y+(h-im.height)//2))
    return sheet

def single(sheet,button,factor):
    result=Image.new('RGBA',(32*factor,32*factor));x,y,w,h=CELLS[button]
    result.paste(sheet.crop((x*factor,y*factor,(x+30)*factor,(y+30)*factor)),(factor,factor));return result

def dpad(paths,images,factor):
    result=Image.new('RGBA',(32*factor,16*factor));draw=ImageDraw.Draw(result)
    for i,path in enumerate(paths[9:11]):
        x=i*16*factor;draw.rounded_rectangle((x+factor,factor,x+15*factor-1,15*factor-1),radius=2*factor,fill=(15,23,35,245))
        im=images[path].copy();im.thumbnail((12*factor,12*factor),Image.Resampling.LANCZOS);result.alpha_composite(im,(x+(16*factor-im.width)//2,(16*factor-im.height)//2))
    return result

def ps2(sheet,swapped,factor):
    # Match the original controls mapper: source physical RT, RB, LB, Y,
    # unused white, LT, X, A -> logical B A X Y white black LT RT.
    out=sheet.copy()
    for x,y,w,h in [(0,0,128,30),(0,30,62,34),(62,29,18,35)]:out.paste((0,0,0,0),(x*factor,y*factor,(x+w)*factor,(y+h)*factor))
    for dest,src in enumerate([7,5,4,3,4,6,2,1]):
        if dest==4:continue
        if swapped:src={0:1,1:0,2:3,3:2}.get(src,src)
        dx,dy,dw,dh=CELLS[dest];sx,sy,sw,sh=CELLS[src]
        w=dw*factor;h=sh*w//sw
        if h>dh*factor:h=dh*factor;w=sw*h//sh
        im=sheet.crop((sx*factor,sy*factor,(sx+sw)*factor,(sy+sh)*factor))
        im=im.resize((w,h),Image.Resampling.NEAREST if factor==1 else Image.Resampling.LANCZOS)
        out.paste(im,(dx*factor+(dw*factor-w)//2,dy*factor+(dh*factor-h)//2))
    return out

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,default=ART/'generated');args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
    provenance=json.loads((ART/'PROVENANCE.json').read_text());inputs={r['archive_path']:ART/r['path'] for r in provenance['files']}
    for r in provenance['files']:assert hashlib.sha256((ART/r['path']).read_bytes()).hexdigest()==r['sha256']
    with tempfile.TemporaryDirectory(prefix='gamepkg-kenney-render-') as temp:
        temp=Path(temp)
        def render(item):
            name,path=item;out=temp/(hashlib.sha256(name.encode()).hexdigest()+'.png')
            result=subprocess.run(['inkscape',str(path),'--export-type=png','--export-width=256','--export-filename='+str(out)],capture_output=True)
            if result.returncode:raise RuntimeError(result.stderr.decode())
            im=Image.open(out).convert('RGBA');box=im.getchannel('A').getbbox();assert box;return name,im.crop(box)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:images=dict(pool.map(render,inputs.items()))
    families=choices();table=[[[None]*4 for _ in range(4)] for _ in range(30)];normal={};preview=[]
    for factor_i,factor in enumerate([1,2,3,4]):
        for family_i,paths in enumerate(families):
            if paths is None:continue
            sheet=compose(paths,images,factor,original=family_i in (0,10));normal[family_i,factor]=sheet
            index=family_i-1 if family_i else 29
            for tex,im in enumerate([sheet,single(sheet,1,factor),single(sheet,0,factor),dpad(paths,images,factor)]):table[index][tex][factor_i]=im
            if factor==4:preview.append((family_i,sheet.copy()))
        for family_i in range(15):
            if family_i==9:continue # keyboard never follows the PS2 driving remap
            source_i=14 if family_i in (0,5,10) else family_i
            if (source_i,factor) not in normal:continue # accurate PlayStation fallback
            sheet=ps2(normal[source_i,factor],source_i in SWAPPED,factor);index=14+family_i
            for tex,im in enumerate([sheet,single(sheet,1,factor),single(sheet,0,factor),table[source_i-1 if source_i else 29][3][factor_i]]):table[index][tex][factor_i]=im
    unique={};refs={};data={f: ['#include \"prompt_art.h\"\n'] for f in [1,2,3,4]};parts=['/* Generated from Kenney CC0 vectors by tools/build_kenney_prompts.py. */\n#include "prompt_art.h"\n']
    for vi,variant in enumerate(table):
        for ti,texture in enumerate(variant):
            for fi,im in enumerate(texture):
                if im is None:continue
                a=np.asarray(im).astype(np.uint32);pixels=((a[...,3]<<24)|(a[...,0]<<16)|(a[...,1]<<8)|a[...,2]).reshape(-1)
                key=(im.width,im.height,hashlib.sha256(pixels.astype('<u4').tobytes()).hexdigest())
                if key not in unique:
                    number=len(unique);unique[key]=number;starts=np.r_[0,np.flatnonzero(pixels[1:]!=pixels[:-1])+1];ends=np.r_[starts[1:],len(pixels)];runs=[]
                    for begin,end in zip(starts,ends):runs.extend([int(end-begin),int(pixels[begin])])
                    body=','.join(f'0x{x:08x}u' for x in runs)
                    data[fi+1].append(f'static const uint32_t r{number}[]={{'+body+'};\n')
                    data[fi+1].append(f'const struct gamepkg_prompt_rle i{number}={{ {im.width}, {im.height}, {len(runs)//2}, r{number} }};\n')
                refs[vi,ti,fi]=unique[key]
    for number in unique.values():parts.append(f'extern const struct gamepkg_prompt_rle i{number};\n')
    for factor,content in data.items():(args.output/f'prompt_art_{factor}x.c').write_text(''.join(content))
    parts.append('const struct gamepkg_prompt_rle *const gamepkg_kenney_art[30][4][4]={\n')
    for vi in range(30):
        parts.append('{'+','.join('{'+','.join('&i'+str(refs[vi,ti,fi]) if (vi,ti,fi) in refs else '0' for fi in range(4))+'}' for ti in range(4))+'},\n')
    parts.append('};\n');(args.output/'prompt_art.c').write_text(''.join(parts))
    sheet=Image.new('RGBA',(1024,280*((len(preview)+1)//2)),(20,26,38,255));d=ImageDraw.Draw(sheet)
    for i,(family_i,im) in enumerate(preview):x=i%2*512;y=i//2*280;sheet.alpha_composite(im,(x,y+24));d.text((x+10,y+5),f'Controller glyph set {family_i}',fill='white')
    sheet.convert('RGB').save(args.output/'preview.png')
    (args.output/'manifest.json').write_text(json.dumps({'generator':'tools/build_kenney_prompts.py','license':'CC0-1.0','families':list(normal_key[0] for normal_key in normal if normal_key[1]==1),'unique_images':len(unique),'factors':[1,2,3,4],'sources':{p.name:{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size} for p in sorted(args.output.glob('prompt_art*.c'))}},indent=2)+'\n')
    print((args.output/'manifest.json').read_text())
if __name__=='__main__':main()
