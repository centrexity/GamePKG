"""Capture only a controlled native GamePKG window and verify guest shader input."""
import ctypes,json,math,os,struct,subprocess,time,zlib
from ctypes import wintypes as W
from pathlib import Path
try:
 from PIL import Image
except ModuleNotFoundError:
 class Image:
  # The Windows capture is raw BGRX DIB pixels; no image package is needed to
  # wrap, inspect or save them. Keep this helper usable on bare Python hosts.
  def __init__(self,width,height,pixels):
   self.width,self.height,self._pixels,self.info=width,height,pixels,{}
  @property
  def size(self):return self.width,self.height
  @staticmethod
  def frombytes(mode,size,data,*args):
   assert mode=='RGB' and args[-1]=='BGRX'
   width,height=size
   pixels=bytearray(width*height*3)
   for i in range(width*height):
    j,k=i*4,i*3
    pixels[k],pixels[k+1],pixels[k+2]=data[j+2],data[j+1],data[j]
   return Image(width,height,pixels)
  def crop(self,box):
   left,top,right,bottom=box
   stride=(right-left)*3;pixels=bytearray()
   for y in range(top,bottom):
    start=(y*self.width+left)*3;pixels+=self._pixels[start:start+stride]
   return Image(right-left,bottom-top,pixels)
  def getdata(self):
   pixels=self._pixels
   return (tuple(pixels[i:i+3]) for i in range(0,len(pixels),3))
  def getpixel(self,point):
   x,y=point;offset=(y*self.width+x)*3
   return tuple(self._pixels[offset:offset+3])
  def save(self,path):
   def chunk(tag,data):
    return struct.pack('>I',len(data))+tag+data+struct.pack('>I',zlib.crc32(tag+data)&0xffffffff)
   width,height=self.width,self.height
   raw=b''.join(b'\0'+self._pixels[y*width*3:(y+1)*width*3] for y in range(height))
   Path(path).write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,2,0,0,0))+chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b''))

def texture_frame_error(image,channel):
 # Validate the interior of the complete 640x480 game viewport. Ignore only
 # its one-pixel fractional raster boundary, not uncaptured client edges.
 width,height=image.size;scale=min(width/640,height/480)
 left=(width-640*scale)/2;top=(height-480*scale)/2
 box=(math.ceil(left)+1,math.ceil(top)+1,math.floor(left+640*scale)-1,math.floor(top+480*scale)-1)
 image.info.setdefault('window',{})['validated_game_interior']=box
 expected=(0,0,0) if channel is None else ((64,255,0) if channel==1 else (64,0,255))
 tolerance=0 if channel is None else 1
 interior=image.crop(box)
 wrong=next((rgb for rgb in interior.getdata() if any(abs(a-b)>tolerance for a,b in zip(rgb,expected))),None)
 if wrong is not None:return f'Incomplete texture viewport: expected {expected}, got {wrong} in {box}'
 points=[]
 if left>=2:points += [(x,height*y//4) for x in [1,width-2] for y in [1,2,3]]
 if top>=2:points += [(width*x//4,y) for x in [1,2,3] for y in [1,height-2]]
 if any(image.getpixel(point)!=(0,0,0) for point in points):return 'Texture viewport letterbox did not match the client aspect ratio'
 return None

def verify_shader_input(project,env,game_id,label,*,texture=False,renderer="opengl",optional=False):
 assert os.name=='nt'
 user=ctypes.WinDLL('user32',use_last_error=True);gdi=ctypes.WinDLL('gdi32',use_last_error=True)
 callback_type=ctypes.WINFUNCTYPE(W.BOOL,W.HWND,W.LPARAM)
 signatures=[(user,'EnumWindows',[callback_type,W.LPARAM],W.BOOL),(user,'GetWindowThreadProcessId',[W.HWND,ctypes.POINTER(W.DWORD)],W.DWORD),(user,'IsWindowVisible',[W.HWND],W.BOOL),(user,'GetClientRect',[W.HWND,ctypes.POINTER(W.RECT)],W.BOOL),(user,'GetDC',[W.HWND],W.HDC),(user,'ReleaseDC',[W.HWND,W.HDC],ctypes.c_int),(user,'PostMessageW',[W.HWND,W.UINT,W.WPARAM,W.LPARAM],W.BOOL),(user,'SetForegroundWindow',[W.HWND],W.BOOL),(user,'GetForegroundWindow',[],W.HWND),(gdi,'CreateCompatibleDC',[W.HDC],W.HDC),(gdi,'SelectObject',[W.HDC,W.HGDIOBJ],W.HGDIOBJ),(gdi,'DeleteObject',[W.HGDIOBJ],W.BOOL),(gdi,'DeleteDC',[W.HDC],W.BOOL),(gdi,'BitBlt',[W.HDC,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_int,W.HDC,ctypes.c_int,ctypes.c_int,W.DWORD],W.BOOL)]
 for library,name,arguments,result in signatures:
  function=getattr(library,name);function.argtypes=arguments;function.restype=result
 for name,arguments,result in [('ShowWindow',[W.HWND,ctypes.c_int],W.BOOL),('SetWindowPos',[W.HWND,W.HWND,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_int,W.UINT],W.BOOL),('ClientToScreen',[W.HWND,ctypes.POINTER(W.POINT)],W.BOOL),('SetCursorPos',[ctypes.c_int,ctypes.c_int],W.BOOL),('WindowFromPoint',[W.POINT],W.HWND)]:
  function=getattr(user,name);function.argtypes=arguments;function.restype=result
 user.SetThreadDpiAwarenessContext.argtypes=[W.HANDLE];user.SetThreadDpiAwarenessContext.restype=W.HANDLE
 user.GetDpiForWindow.argtypes=[W.HWND];user.GetDpiForWindow.restype=W.UINT
 user.GetWindowRect.argtypes=[W.HWND,ctypes.POINTER(W.RECT)];user.GetWindowRect.restype=W.BOOL
 user.GetSystemMetrics.argtypes=[ctypes.c_int];user.GetSystemMetrics.restype=ctypes.c_int
 class MouseInput(ctypes.Structure):
  _fields_=[('dx',W.LONG),('dy',W.LONG),('data',W.DWORD),('flags',W.DWORD),('time',W.DWORD),('extra',W.WPARAM)]
 class KeyboardInput(ctypes.Structure):
  _fields_=[('vk',W.WORD),('scan',W.WORD),('flags',W.DWORD),('time',W.DWORD),('extra',W.WPARAM)]
 class HardwareInput(ctypes.Structure):
  _fields_=[('message',W.DWORD),('low',W.WORD),('high',W.WORD)]
 class InputData(ctypes.Union):
  _fields_=[('mouse',MouseInput),('keyboard',KeyboardInput),('hardware',HardwareInput)]
 class Input(ctypes.Structure):
  _fields_=[('kind',W.DWORD),('data',InputData)]
 assert ctypes.sizeof(Input)==(40 if ctypes.sizeof(ctypes.c_void_p)==8 else 28)
 user.SendInput.argtypes=[W.UINT,ctypes.POINTER(Input),ctypes.c_int];user.SendInput.restype=W.UINT
 def key(scan,release):
  event=Input();event.kind=1
  event.data.keyboard=KeyboardInput(0,scan,0x8|0x1|(0x2 if release else 0),0,0)
  assert user.SendInput(1,ctypes.byref(event),ctypes.sizeof(event))==1,ctypes.get_last_error()
 class Header(ctypes.Structure):
  _fields_=[('size',W.DWORD),('width',W.LONG),('height',W.LONG),('planes',W.WORD),('bits',W.WORD),('compression',W.DWORD),('image_size',W.DWORD),('x',W.LONG),('y',W.LONG),('colors',W.DWORD),('important',W.DWORD)]
 class Info(ctypes.Structure):_fields_=[('header',Header),('colors',W.DWORD*3)]
 gdi.CreateDIBSection.argtypes=[W.HDC,ctypes.POINTER(Info),W.UINT,ctypes.POINTER(ctypes.c_void_p),W.HANDLE,W.DWORD];gdi.CreateDIBSection.restype=W.HBITMAP
 def capture(window):
  rectangle=W.RECT();assert user.GetClientRect(window,ctypes.byref(rectangle))
  width,height=rectangle.right,rectangle.bottom;assert 1<=width<=4096 and 1<=height<=4096,f'Unexpected native client area: {width}x{height}'
  dc=user.GetDC(window);memory=gdi.CreateCompatibleDC(dc);assert dc and memory
  info=Info();info.header=Header(ctypes.sizeof(Header),width,-height,1,32,0,0,0,0,0,0)
  pixels=ctypes.c_void_p();bitmap=gdi.CreateDIBSection(dc,ctypes.byref(info),0,ctypes.byref(pixels),None,0);assert bitmap and pixels.value
  previous=gdi.SelectObject(memory,bitmap)
  try:
   assert gdi.BitBlt(memory,0,0,width,height,dc,0,0,0x00CC0020)
   image=Image.frombytes('RGB',(width,height),ctypes.string_at(pixels,width*height*4),'raw','BGRX')
   outer=W.RECT();assert user.GetWindowRect(window,ctypes.byref(outer))
   image.info['window']={'renderer':renderer,'client_pixels':[width,height],'outer_pixels':[outer.left,outer.top,outer.right,outer.bottom],'dpi':user.GetDpiForWindow(window),'capture_thread_dpi':'per-monitor-v2','screen_pixels':[user.GetSystemMetrics(0),user.GetSystemMetrics(1)]}
   return image
  finally:gdi.SelectObject(memory,previous);gdi.DeleteObject(bitmap);gdi.DeleteDC(memory);user.ReleaseDC(window,dc)
 proof=Path(project)/'target/windows-gui-proof';proof.mkdir(parents=True,exist_ok=True)
 def evidence(name,image):
  image.save(proof/(label+'-'+name+'.png'))
  (proof/(label+'-'+name+'.json')).write_text(json.dumps(image.info['window'],indent=2)+'\n')
 log_path=proof/(label+'.log');window=None;key_held=False;old_awareness=None
 assert renderer in ['opengl','vulkan']
 player_env=dict(env,GALLIUM_DRIVER='llvmpipe',LIBGL_ALWAYS_SOFTWARE='1',GAMEPKG_RENDERER=renderer)
 player_env.pop('GAMEPKG_SLANG_ROOT',None)
 with log_path.open('wb') as log:
  # WSL-compiled fixtures are Wasm packages; explicitly select the Wasm player
  # so native admission never rejects an unmigrated core/component manifest.
  app=subprocess.Popen([str(Path(project)/'target/release/gamepkg-gui.exe'),'--play',game_id,'--target','wasm'],env=player_env,stdout=log,stderr=log)
  def renderer_unavailable():
   text=log_path.read_text(errors='replace').lower()
   return any(marker in text for marker in [
    'egui_glow requires opengl 2.0+','no suitable adapter',
    'failed to create wgpu','could not find a suitable graphics adapter',
    'vulkan loader','failed to load vulkan','failedtocreatesurface',
    'failed to create surface',
   ]) or ('wgpu' in text and 'adapter' in text)
  try:
   # Query/capture physical client pixels consistently with winit. This only
   # changes this test thread, and the original context is restored below.
   # https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setthreaddpiawarenesscontext
   old_awareness=user.SetThreadDpiAwarenessContext(W.HANDLE(-4))
   assert old_awareness,'Could not select per-monitor-v2 capture coordinates'
   deadline=time.monotonic()+30
   while window is None:
    if app.poll() is not None:
     if optional and renderer_unavailable():
      print(f'Native Windows GUI renderer {renderer} unavailable; skipping {label} pixel proof',flush=True)
      return False
     assert app.poll() is None,log_path.read_text(errors='replace')
    found=[]
    @callback_type
    def visit(handle,_):
     pid=W.DWORD();user.GetWindowThreadProcessId(handle,ctypes.byref(pid))
     if pid.value==app.pid and user.IsWindowVisible(handle):
      rectangle=W.RECT()
      if user.GetClientRect(handle,ctypes.byref(rectangle)):
       width,height=rectangle.right,rectangle.bottom
       # Skip small transient windows created during graphics initialization.
       if 320<=width<=4096 and 240<=height<=4096:found.append((width*height,handle))
     return True
    user.EnumWindows(visit,0)
    if found:window=max(found)[1];break
    assert time.monotonic()<deadline,'Native GamePKG window did not appear';time.sleep(.1)
   # Background automation cannot always call SetForegroundWindow directly.
   # Activate our own visible client through a real click, as a user would.
   # https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setforegroundwindow
   user.ShowWindow(window,9)
   assert user.SetWindowPos(window,W.HWND(-1),0,0,0,0,0x1|0x10|0x40)
   try:
    rectangle=W.RECT();assert user.GetClientRect(window,ctypes.byref(rectangle))
    point=W.POINT(rectangle.right//2,rectangle.bottom//2)
    assert user.ClientToScreen(window,ctypes.byref(point))
    deadline=time.monotonic()+10
    while user.WindowFromPoint(point)!=window:
     assert time.monotonic()<deadline,'Native game client was not available for activation'
     time.sleep(.1)
    assert user.SetCursorPos(point.x,point.y)
    events=(Input*2)()
    events[0].data.mouse.flags=0x2;events[1].data.mouse.flags=0x4
    assert user.SendInput(2,events,ctypes.sizeof(Input))==2,ctypes.get_last_error()
   finally:
    assert user.SetWindowPos(window,W.HWND(-2),0,0,0,0,0x1|0x2|0x10)
   deadline=time.monotonic()+10
   while user.GetForegroundWindow()!=window:
    user.SetForegroundWindow(window)
    assert time.monotonic()<deadline,'Native game could not receive keyboard focus'
    time.sleep(.1)
   def require_color(channel,name):
    deadline=time.monotonic()+30
    while True:
     assert app.poll() is None,log_path.read_text(errors='replace')
     image=capture(window)
     error=texture_frame_error(image,channel) if texture else None
     evidence(name,image)
     count=sum(1 for rgb in image.getdata() if rgb[channel]>150 and rgb[channel]>2*rgb[1 if channel==2 else 2])
     if count>20000 and error is None:return
     assert time.monotonic()<deadline,f'Native {name} guest pixels missing or incomplete: {count}; {error}';time.sleep(.1)
   require_color(2,'blue')
   # Exercise the Windows input queue and keyboard state. Posting WM_KEYDOWN
   # directly does not establish a real held key for the native event loop.
   assert user.GetForegroundWindow()==window,'Native game lost keyboard focus'
   key(0x4d,False);key_held=0x4d
   require_color(1,'green')
   key(0x4d,True);key_held=False
   require_color(2,'released-blue')
   if texture:
    # Replacing the image by a plain draw must remove the previous texture.
    key(0x50,False);key_held=0x50
    deadline=time.monotonic()+30
    while True:
     assert app.poll() is None,log_path.read_text(errors='replace')
     image=capture(window);error=texture_frame_error(image,None);evidence('plain-black',image)
     samples=[image.getpixel((image.width*x//4,image.height*y//4)) for x in [1,2,3] for y in [1,2,3]]
     if all(rgb==(0,0,0) for rgb in samples) and error is None:break
     assert time.monotonic()<deadline,f'Plain shader retained a stale texture or incomplete viewport: {samples}; {error}';time.sleep(.1)
    key(0x50,True);key_held=False
    require_color(2,'texture-restored-blue')
    assert user.SetWindowPos(window,None,0,0,853,641,0x2|0x4|0x10)
    require_color(2,'resized-blue')
   assert user.PostMessageW(window,0x10,0,0),'Could not request normal game close'
   assert app.wait(timeout=20)==0,log_path.read_text(errors='replace')
   print('Native Windows shader pixels, held/released keyboard input and normal close passed:',label)
  finally:
   if key_held:key(key_held,True)
   if app.poll() is None:
    if window:user.PostMessageW(window,0x10,0,0)
    try:app.wait(timeout=10)
    except subprocess.TimeoutExpired:app.terminate();app.wait(timeout=10)
   if old_awareness:assert user.SetThreadDpiAwarenessContext(old_awareness)
  return True
