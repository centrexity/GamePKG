"""Capture only a controlled native GamePKG window and verify guest shader input."""
import ctypes,os,subprocess,time
from ctypes import wintypes as W
from pathlib import Path
from PIL import Image

def verify_shader_input(project,env,game_id,label):
 assert os.name=='nt'
 user=ctypes.WinDLL('user32',use_last_error=True);gdi=ctypes.WinDLL('gdi32',use_last_error=True)
 callback_type=ctypes.WINFUNCTYPE(W.BOOL,W.HWND,W.LPARAM)
 signatures=[(user,'EnumWindows',[callback_type,W.LPARAM],W.BOOL),(user,'GetWindowThreadProcessId',[W.HWND,ctypes.POINTER(W.DWORD)],W.DWORD),(user,'IsWindowVisible',[W.HWND],W.BOOL),(user,'GetClientRect',[W.HWND,ctypes.POINTER(W.RECT)],W.BOOL),(user,'GetDC',[W.HWND],W.HDC),(user,'ReleaseDC',[W.HWND,W.HDC],ctypes.c_int),(user,'PostMessageW',[W.HWND,W.UINT,W.WPARAM,W.LPARAM],W.BOOL),(user,'SetForegroundWindow',[W.HWND],W.BOOL),(gdi,'CreateCompatibleDC',[W.HDC],W.HDC),(gdi,'SelectObject',[W.HDC,W.HGDIOBJ],W.HGDIOBJ),(gdi,'DeleteObject',[W.HGDIOBJ],W.BOOL),(gdi,'DeleteDC',[W.HDC],W.BOOL),(gdi,'BitBlt',[W.HDC,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_int,W.HDC,ctypes.c_int,ctypes.c_int,W.DWORD],W.BOOL)]
 for library,name,arguments,result in signatures:
  function=getattr(library,name);function.argtypes=arguments;function.restype=result
 class Header(ctypes.Structure):
  _fields_=[('size',W.DWORD),('width',W.LONG),('height',W.LONG),('planes',W.WORD),('bits',W.WORD),('compression',W.DWORD),('image_size',W.DWORD),('x',W.LONG),('y',W.LONG),('colors',W.DWORD),('important',W.DWORD)]
 class Info(ctypes.Structure):_fields_=[('header',Header),('colors',W.DWORD*3)]
 gdi.CreateDIBSection.argtypes=[W.HDC,ctypes.POINTER(Info),W.UINT,ctypes.POINTER(ctypes.c_void_p),W.HANDLE,W.DWORD];gdi.CreateDIBSection.restype=W.HBITMAP
 def capture(window):
  rectangle=W.RECT();assert user.GetClientRect(window,ctypes.byref(rectangle))
  width,height=rectangle.right,rectangle.bottom;assert 1<=width<=4096 and 1<=height<=4096
  dc=user.GetDC(window);memory=gdi.CreateCompatibleDC(dc);assert dc and memory
  info=Info();info.header=Header(ctypes.sizeof(Header),width,-height,1,32,0,0,0,0,0,0)
  pixels=ctypes.c_void_p();bitmap=gdi.CreateDIBSection(dc,ctypes.byref(info),0,ctypes.byref(pixels),None,0);assert bitmap and pixels.value
  previous=gdi.SelectObject(memory,bitmap)
  try:
   assert gdi.BitBlt(memory,0,0,width,height,dc,0,0,0x00CC0020)
   return Image.frombytes('RGB',(width,height),ctypes.string_at(pixels,width*height*4),'raw','BGRX')
  finally:gdi.SelectObject(memory,previous);gdi.DeleteObject(bitmap);gdi.DeleteDC(memory);user.ReleaseDC(window,dc)
 proof=Path(project)/'target/windows-gui-proof';proof.mkdir(parents=True,exist_ok=True)
 log_path=proof/(label+'.log');window=None
 player_env=dict(env,GALLIUM_DRIVER='llvmpipe',LIBGL_ALWAYS_SOFTWARE='1')
 player_env.pop('GAMEPKG_SLANG_ROOT',None)
 with log_path.open('wb') as log:
  app=subprocess.Popen([str(Path(project)/'target/release/gamepkg-gui.exe'),'--play',game_id],env=player_env,stdout=log,stderr=log)
  try:
   deadline=time.monotonic()+30
   while window is None:
    assert app.poll() is None,log_path.read_text(errors='replace')
    found=[]
    @callback_type
    def visit(handle,_):
     pid=W.DWORD();user.GetWindowThreadProcessId(handle,ctypes.byref(pid))
     if pid.value==app.pid and user.IsWindowVisible(handle):found.append(handle)
     return True
    user.EnumWindows(visit,0)
    if found:window=found[0];break
    assert time.monotonic()<deadline,'Native GamePKG window did not appear';time.sleep(.1)
   user.SetForegroundWindow(window)
   def require_color(channel,name):
    deadline=time.monotonic()+30
    while True:
     assert app.poll() is None,log_path.read_text(errors='replace')
     image=capture(window);image.save(proof/(label+'-'+name+'.png'))
     count=sum(1 for rgb in image.getdata() if rgb[channel]>150 and rgb[channel]>2*rgb[1 if channel==2 else 2])
     if count>20000:return
     assert time.monotonic()<deadline,f'Native {name} guest pixels missing: {count}';time.sleep(.1)
   require_color(2,'blue')
   assert user.PostMessageW(window,0x100,0x27,(0x4d<<16)|(1<<24)|1)
   require_color(1,'green')
   assert user.PostMessageW(window,0x101,0x27,(0x4d<<16)|(1<<24)|(3<<30)|1)
   require_color(2,'released-blue')
   print('Native Windows shader pixels and held/released keyboard input passed:',label)
  finally:
   if app.poll() is None:
    if window:user.PostMessageW(window,0x10,0,0)
    try:app.wait(timeout=10)
    except subprocess.TimeoutExpired:app.terminate();app.wait(timeout=10)
