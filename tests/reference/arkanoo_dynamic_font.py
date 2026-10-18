"""Pinned original dynamic Roboto reference and dependency-free font data."""
from pathlib import Path
import argparse,hashlib,json,tempfile,zlib
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);parser.add_argument('--write-source',type=Path);args=parser.parse_args()

from fontTools.ttLib import TTFont
import subprocess,struct,unicodedata,regex
font=str(args.original/'assets/font/static/Roboto-Medium.ttf')
assert hashlib.sha256(Path(font).read_bytes()).hexdigest()=='616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'
assert hashlib.sha256((args.original/'src/rendering.rs').read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
work=tempfile.TemporaryDirectory(prefix='gamepkg-dynamic-font-');exe=str(Path(work.name)/'capture')
subprocess.run(['cc',Path(__file__).with_name('sdl_dynamic_font.c'),'-o',exe,*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf','harfbuzz','freetype2'],text=True).split()],check=True)
cm=TTFont(font);cmap={cp:cm.getGlyphID(name) for cp,name in cm.getBestCmap().items()}

raw=subprocess.check_output([exe,font,'glyphs']);count,asc,height=struct.unpack_from('<3i',raw);at=12;glyphs=[]
for i in range(count):
 left,top,w,h,advance=struct.unpack_from('<5i',raw,at);at+=20;alpha=raw[at:at+w*h];at+=w*h;glyphs.append((left,top,w,h,advance,alpha))
assert at==len(raw)
space_glyphs={0x202f:count,0x205f:count+1,0x3000:count+2}
for cp in space_glyphs:
 spacing_capture=subprocess.check_output([exe,font,'shape',chr(cp)]);assert struct.unpack_from('<i',spacing_capture)[0]==1
 spacing=struct.unpack_from('<i',spacing_capture,16)[0];glyphs.append((0,0,0,0,spacing,b""))
glyphs.append((0,0,0,0,0,b""));invisible_id=len(glyphs)-1
kern_units={};seen=set()
for st in cm['GPOS'].table.LookupList.Lookup[4].SubTable:
 if st.Format==1:
  for name,group in zip(st.Coverage.glyphs,st.PairSet):
   for rec in group.PairValueRecord:
    key=(cm.getGlyphID(name),cm.getGlyphID(rec.SecondGlyph));seen.add(key);kern_units[key]=rec.Value1.XAdvance
 else:
  for name in st.Coverage.glyphs:
   cl=st.ClassDef1.classDefs.get(name,0)
   for other in cm.getGlyphOrder():
    key=(cm.getGlyphID(name),cm.getGlyphID(other));value=st.Class1Record[cl].Class2Record[st.ClassDef2.classDefs.get(other,0)].Value1
    if key not in seen and value:kern_units[key]=value.XAdvance
kern={key:(units*1664+1024)//2048 for key,units in kern_units.items() if units}
marks={cm.getGlyphID(name) for name,cl in cm['GDEF'].table.GlyphClassDef.classDefs.items() if cl==3}
ignorable_cache={}
def captured_ignorable(cp):
 if not regex.fullmatch(r'\p{Default_Ignorable_Code_Point}',chr(cp)):return False
 if cp not in ignorable_cache:
  captured=subprocess.check_output([exe,font,'shape','a'+chr(cp)+'b']);n=struct.unpack_from('<i',captured)[0]
  middle=struct.unpack_from('<5i',captured,24) if n==3 else (0,0,0,1,0)
  ignorable_cache[cp]=n==3 and middle[0]==4 and middle[3]==0
 return ignorable_cache[cp]
def norm(text):
 out=[]
 def append(c):
  out.append(c);at=len(out)-1;cc=unicodedata.combining(chr(c&0x1fffff))
  if cc:
   while at and unicodedata.combining(chr(out[at-1]&0x1fffff))>cc:out[at-1],out[at]=out[at],out[at-1];at-=1
 def decomp(c):
  d=unicodedata.decomposition(chr(c));parts=[] if not d or d.startswith('<') else list(map(lambda t:int(t,16),d.split()))
  if parts and all(cp in cmap for cp in parts):
   for cp in parts:decomp(cp)
  else:append(c)
 chars=list(map(ord,text));at=0
 while at<len(chars):
  end=at+1
  while end<len(chars) and unicodedata.category(chr(chars[end])).startswith('M'):end+=1
  raw=any((0x180b<=cp<=0x180d or cp==0x180f or 0xfe00<=cp<=0xfe0f or 0xe0100<=cp<=0xe01ef) for cp in chars[at:end])
  for cp in chars[at:end]:
   if raw:append(cp|0x80000000)
   else:decomp(cp)
  at=end
 composed=[];starter=0;previous=0
 for c in out:
  cp=c&0x1fffff;cc=unicodedata.combining(chr(cp));combo=unicodedata.normalize('NFC',chr(composed[starter]&0x1fffff)+chr(cp)) if composed else ''
  if composed and (previous<cc or previous==0) and len(combo)==1 and ord(combo) in cmap:composed[starter]=ord(combo)|(composed[starter]&0x80000000);continue
  if cc==0:starter=len(composed)
  composed.append(c);previous=cc
 ids=[]
 for at,c in enumerate(composed):
  cp=c&0x1fffff
  if captured_ignorable(cp):
   flags=0x10000000 if cp==0x200c else 0
   if cp==0x034f and at and at+1<len(composed):
    prev=unicodedata.combining(chr(composed[at-1]&0x1fffff));following=unicodedata.combining(chr(composed[at+1]&0x1fffff))
    if following and prev>following:flags|=0x20000000
   ids.append(invisible_id|flags)
  else:ids.append(cmap.get(cp,0) if c&0x80000000 else space_glyphs.get(cp,cmap.get(cp,0)))
 lookup=cm['GSUB'].table.LookupList.Lookup;context=lookup[2].SubTable[0];above={cm.getGlyphID(n) for n,c in context.LookAheadClassDef.classDefs.items() if c==1};mapping={cm.getGlyphID(a):cm.getGlyphID(b) for a,b in lookup[3].SubTable[0].mapping.items()}
 def hidden(gid):return gid&0xffff==invisible_id
 for i in range(len(ids)):
  following=i+1
  while following<len(ids) and hidden(ids[following]) and not ids[following]&0x20000000:following+=1
  if following<len(ids) and ids[following] in above:ids[i]=mapping.get(ids[i],ids[i])
 for i in range(len(ids)):
  if i>=len(ids):break
  rules=[] if ids[i]>=count else lookup[4].SubTable[0].ligatures.get(cm.getGlyphName(ids[i]),[])
  for rule in rules:
   components=[cm.getGlyphID(n) for n in rule.Component];matched=[];following=i+1
   for wanted in components:
    while following<len(ids) and hidden(ids[following]) and not ids[following]&0x30000000:following+=1
    if following>=len(ids) or ids[following]!=wanted:break
    matched.append(following);following+=1
   if len(matched)==len(components):
    ids[i]=cm.getGlyphID(rule.LigGlyph)
    for removed in reversed(matched):ids.pop(removed)
    break
 return ids
def model(text):
 x=0;minx=0;maxx=0;miny=0;maxy=height;placed=[]
 ids=norm(text)
 for i,idx in enumerate(ids):
  ignorable=idx&0xffff==invisible_id
  kern_adjust=0
  if idx not in marks and idx&0xffff!=invisible_id:
   following=i+1
   while following<len(ids) and (ids[following] in marks or ids[following]&0xffff==invisible_id and not ids[following]&0x20000000):following+=1
   if following<len(ids):kern_adjust=kern.get((idx,ids[following]),0)
  l,t,w,h,advance,a=glyphs[idx&0xffff];placed.append((idx,x//64,asc));minx=min(minx,x//64+l);maxx=max(maxx,x//64+l+w);miny=min(miny,asc-t);maxy=max(maxy,asc-t+h);x+=0 if ignorable else advance+kern_adjust
 maxx=max(maxx,x//64);w=maxx-minx;h=maxy-miny;p=bytearray([200,200,200,0])*(w*h)
 for idx,x,y in placed:
  l,t,gw,gh,adv,a=glyphs[idx&0xffff]
  for row in range(gh):
   for col in range(gw):p[((y-t-miny+row)*w+x+l-minx+col)*4+3]|=a[row*gw+col]
 return w,h,p

if args.write_source:
 tables={};meta=bytearray();encoded=bytearray()
 for left,top,w,h,advance,alpha in glyphs:
  runs=[]
  for value in alpha:
   if runs and runs[-1][1]==value and runs[-1][0]<255:runs[-1][0]+=1
   else:runs.append([1,value])
  packed=bytes(v for pair in runs for v in pair)
  meta.extend(struct.pack('<hhHHIII',left,top,w,h,advance,len(encoded),len(packed)));encoded.extend(packed)
 tables['atlas_metadata']=bytes(meta);tables['atlas_pixels']=bytes(encoded)
 tables['space_fallback']=b''.join(struct.pack('<II',cp,gid) for cp,gid in sorted(space_glyphs.items()))
 tables['cmap']=b''.join(struct.pack('<II',cp,gid) for cp,gid in sorted(cmap.items()))
 decomps=[];composes=[];classes=[];ignore=[];start=None;mark_ranges=[];mark_start=None
 for cp in range(0x110000):
  ch=chr(cp);d=unicodedata.decomposition(ch)
  if d and not d.startswith('<'):
   parts=[int(v,16) for v in d.split()];assert len(parts)<=2
   decomps.append((cp,parts[0],parts[1] if len(parts)>1 else 0))
   if len(parts)==2 and unicodedata.normalize('NFC',''.join(map(chr,parts)))==ch:composes.append((*parts,cp))
  if unicodedata.combining(ch):classes.append((cp,unicodedata.combining(ch)))
  is_mark=unicodedata.category(ch).startswith('M')
  if is_mark and mark_start is None:mark_start=cp
  if not is_mark and mark_start is not None:mark_ranges.append((mark_start,cp-1));mark_start=None
  invisible=captured_ignorable(cp)
  if invisible and start is None:start=cp
  if not invisible and start is not None:ignore.append((start,cp-1));start=None
 if start is not None:ignore.append((start,0x10ffff))
 tables['decomp']=b''.join(struct.pack('<III',*v) for v in decomps)
 tables['compose']=b''.join(struct.pack('<III',*v) for v in sorted(composes))
 tables['classes']=b''.join(struct.pack('<II',*v) for v in classes)
 tables['mark_chars']=b''.join(struct.pack('<II',*v) for v in mark_ranges)
 tables['ignore']=b''.join(struct.pack('<II',*v) for v in ignore)
 context=cm['GSUB'].table.LookupList.Lookup[2].SubTable[0]
 context_map=cm['GSUB'].table.LookupList.Lookup[3].SubTable[0].mapping
 tables['context_map']=b''.join(struct.pack('<II',cm.getGlyphID(a),cm.getGlyphID(b)) for a,b in sorted(context_map.items(),key=lambda kv:cm.getGlyphID(kv[0])))
 context_marks=sorted(cm.getGlyphID(n) for n,c in context.LookAheadClassDef.classDefs.items() if c==1)
 tables['context_marks']=b''.join(struct.pack('<II',v,1) for v in context_marks)
 ligatures=[]
 for name,rules in cm['GSUB'].table.LookupList.Lookup[4].SubTable[0].ligatures.items():
  for rule in rules:
   ids=[cm.getGlyphID(n) for n in rule.Component];assert 1<=len(ids)<=2
   ligatures.append((cm.getGlyphID(name),ids[0],ids[1] if len(ids)>1 else 0,cm.getGlyphID(rule.LigGlyph)))
 tables['ligatures']=b''.join(struct.pack('<IIII',*v) for v in sorted(ligatures,key=lambda v:v[0]))
 tables['kern']=b''.join(struct.pack('<IIi',a,b,v) for (a,b),v in sorted(kern.items()))
 tables['marks']=b''.join(struct.pack('<II',v,1) for v in sorted(marks))
 result='// Generated pinned Roboto Medium 26 glyphs and layout data.\n// Roboto Project Authors, SIL OFL 1.1; assets/LICENSE-ROBOTO.txt.\n// Unicode data: assets/LICENSE-UNICODE.txt.\n'
 result+='pub const UNICODE_GLYPH_COUNT:usize='+str(len(glyphs))+';\npub const UNICODE_ASCENT:i32='+str(asc)+';\npub const UNICODE_HEIGHT:i32='+str(height)+';\n'
 for name,value in tables.items():result+='pub fn unicode_'+name+"()-> &'static [u8] {b\""+''.join('\\x'+format(v,'02x') for v in value)+'\"}\n'
 args.write_source.write_text(result);subprocess.run(['rustfmt','--edition','2021',args.write_source],check=True)
# Every mapped character, combining boundaries and actual kerning pairs.
import random
rng=random.Random(6719)
samples=['Name: my_pattern (N to edit)','Name: café_ (ESC to finish)','AVATAR To WA ffi fj','cafe\u0301','Ångström','Ελληνικά','Привет мир','日本語 🎮','Å','가','a\u200db','a\ufe0fb','//','\"Δ','/\u0301/','/\u200d/']
samples += ['Name: cafe\u0301_ (ESC to finish)','Name: Ελληνικά Привет 日本語 🎮 // “Δ (N to edit)','Name: '+'a'*200+' (N to edit)','Saved: café']
samples += ['my_pattern','cafe\u0301','Ελληνικά Привет 日本語 🎮 // “Δ','a'*200]
samples += ['a'+chr(cp)+'b' for cp in [0x115f,0x1160,0x3164,0xffa0,0x85,0xa0,0x1680,0x180e,*range(0x2000,0x2012),0x2028,0x2029,0x202f,0x205f,0x3000]]
samples += [chr(cp) for cp in cmap if cp!=0]
samples += [''.join(rng.choice([chr(cp) for cp in cmap if cp!=0]) for _ in range(rng.randrange(1,18))) for _ in range(150)]
samples += [base+chr(mark) for base in ['A','a','i','j','é','Ω','Д'] for mark in range(0x300,0x370)]
reverse={gid:cp for cp,gid in cmap.items() if cp!=0}
samples += [chr(reverse[a])+chr(reverse[b]) for a,b in kern if a in reverse and b in reverse]
compound_rng=random.Random(7901)
compound_pool=list('AaeijéΩД')+['\u0300','\u0301','\u0304','\u0307','\u0308','\u0313','\u0327','\u0344','\u034f','\u200c','\u200d','\ufe0f','\u202f','\u205f','\u115f','/','Δ']
samples += ['j\u200d\u0301','i\u200c\u0301','e\u0301\ufe0f','a\u0301\u0327','e\u0301\u0300']+[''.join(compound_rng.choice(compound_pool) for _ in range(compound_rng.randrange(2,15))) for _ in range(1200)]
cases=[]
for text in dict.fromkeys(samples):
 reply=subprocess.run([exe,font,'text',text],capture_output=True);w,h,p=model(text)
 if reply.returncode:assert w==0,(text,w,h);cases.append(dict(text=text,width=0,height=h,rendered=False));continue
 ew,eh=struct.unpack('<2i',reply.stdout[:8]);assert (w,h,bytes(p))==(ew,eh,reply.stdout[8:]),repr(text)
 cases.append(dict(text=text,width=w,height=h,rendered=True,rgba_sha256=hashlib.sha256(p).hexdigest(),rgba_zlib_hex=zlib.compress(p).hex()))
print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',font_size=26,unicode_version=unicodedata.unidata_version,glyph_count=count,mapped_scalars=len(cmap),default_ignorable_candidates=len(ignorable_cache),default_ignorable_hidden=sum(ignorable_cache.values()),tool_versions={name:subprocess.check_output(['pkg-config','--modversion',name],text=True).strip() for name in ['SDL2_ttf','harfbuzz','freetype2']},cases=cases),indent=2));work.cleanup()
