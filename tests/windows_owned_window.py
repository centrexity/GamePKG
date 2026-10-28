"""Maintainer-only Win32 input/capture for a single owned game process."""
import ctypes, json, subprocess, time
from ctypes import wintypes as W
from PIL import Image

class OwnedGameWindow:
 def __init__(self, executable, arguments, environment, proof, label):
  self.executable=executable;self.arguments=arguments;self.environment=environment
  self.proof=proof;self.label=label;self.app=None;self.window=None;self.pressed=set()
  self.proof.mkdir(parents=True,exist_ok=True)
  self.user=ctypes.WinDLL('user32',use_last_error=True);self.gdi=ctypes.WinDLL('gdi32',use_last_error=True)
  self.callback_type=ctypes.WINFUNCTYPE(W.BOOL,W.HWND,W.LPARAM)
  signatures=[(self.user,'EnumWindows',[self.callback_type,W.LPARAM],W.BOOL),
   (self.user,'GetWindowThreadProcessId',[W.HWND,ctypes.POINTER(W.DWORD)],W.DWORD),
   (self.user,'IsWindowVisible',[W.HWND],W.BOOL),(self.user,'GetClientRect',[W.HWND,ctypes.POINTER(W.RECT)],W.BOOL),
   (self.user,'GetDC',[W.HWND],W.HDC),(self.user,'ReleaseDC',[W.HWND,W.HDC],ctypes.c_int),
   (self.user,'PostMessageW',[W.HWND,W.UINT,W.WPARAM,W.LPARAM],W.BOOL),
   (self.user,'SetForegroundWindow',[W.HWND],W.BOOL),(self.user,'GetForegroundWindow',[],W.HWND),
   (self.user,'ShowWindow',[W.HWND,ctypes.c_int],W.BOOL),
   (self.user,'SetWindowPos',[W.HWND,W.HWND,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_int,W.UINT],W.BOOL),
   (self.user,'ClientToScreen',[W.HWND,ctypes.POINTER(W.POINT)],W.BOOL),
   (self.user,'SetCursorPos',[ctypes.c_int,ctypes.c_int],W.BOOL),(self.user,'WindowFromPoint',[W.POINT],W.HWND),
   (self.user,'SetThreadDpiAwarenessContext',[W.HANDLE],W.HANDLE),
   (self.user,'GetDpiForWindow',[W.HWND],W.UINT),
   (self.gdi,'CreateCompatibleDC',[W.HDC],W.HDC),(self.gdi,'SelectObject',[W.HDC,W.HGDIOBJ],W.HGDIOBJ),
   (self.gdi,'DeleteObject',[W.HGDIOBJ],W.BOOL),(self.gdi,'DeleteDC',[W.HDC],W.BOOL),
   (self.gdi,'BitBlt',[W.HDC,ctypes.c_int,ctypes.c_int,ctypes.c_int,ctypes.c_int,W.HDC,ctypes.c_int,ctypes.c_int,W.DWORD],W.BOOL)]
  for library,name,arguments,result in signatures:
   fn=getattr(library,name);fn.argtypes=arguments;fn.restype=result
  class Mouse(ctypes.Structure):
   _fields_=[('dx',W.LONG),('dy',W.LONG),('data',W.DWORD),('flags',W.DWORD),('time',W.DWORD),('extra',W.WPARAM)]
  class Keyboard(ctypes.Structure):
   _fields_=[('vk',W.WORD),('scan',W.WORD),('flags',W.DWORD),('time',W.DWORD),('extra',W.WPARAM)]
  class Hardware(ctypes.Structure):_fields_=[('message',W.DWORD),('low',W.WORD),('high',W.WORD)]
  class Data(ctypes.Union):_fields_=[('mouse',Mouse),('keyboard',Keyboard),('hardware',Hardware)]
  class Input(ctypes.Structure):_fields_=[('kind',W.DWORD),('data',Data)]
  class Header(ctypes.Structure):
   _fields_=[('size',W.DWORD),('width',W.LONG),('height',W.LONG),('planes',W.WORD),('bits',W.WORD),('compression',W.DWORD),('image_size',W.DWORD),('x',W.LONG),('y',W.LONG),('colors',W.DWORD),('important',W.DWORD)]
  class Info(ctypes.Structure):_fields_=[('header',Header),('colors',W.DWORD*3)]
  self.Input=Input;self.Keyboard=Keyboard;self.Header=Header;self.Info=Info
  assert ctypes.sizeof(Input)==(40 if ctypes.sizeof(ctypes.c_void_p)==8 else 28)
  self.user.SendInput.argtypes=[W.UINT,ctypes.POINTER(Input),ctypes.c_int];self.user.SendInput.restype=W.UINT
  self.gdi.CreateDIBSection.argtypes=[W.HDC,ctypes.POINTER(Info),W.UINT,ctypes.POINTER(ctypes.c_void_p),W.HANDLE,W.DWORD];self.gdi.CreateDIBSection.restype=W.HBITMAP
 def diagnostics(self):return self.log_path.read_text(errors='replace')[-4000:]
 def alive(self):assert self.app.poll() is None,self.diagnostics()
 def __enter__(self):
  self.log_path=self.proof/(self.label+'.log');self.log=self.log_path.open('wb')
  self.awareness=self.user.SetThreadDpiAwarenessContext(W.HANDLE(-4))
  assert self.awareness,'per-monitor capture coordinates unavailable'
  try:
   self.app=subprocess.Popen([str(self.executable),*self.arguments],env=self.environment,stdout=self.log,stderr=self.log)
   end=time.monotonic()+45
   while self.window is None:
    self.alive();found=[]
    @self.callback_type
    def visit(handle,_):
     pid=W.DWORD();self.user.GetWindowThreadProcessId(handle,ctypes.byref(pid))
     if pid.value==self.app.pid and self.user.IsWindowVisible(handle):
      rect=W.RECT()
      if self.user.GetClientRect(handle,ctypes.byref(rect)) and 320<=rect.right<=4096 and 240<=rect.bottom<=4096:found.append((rect.right*rect.bottom,handle))
     return True
    self.user.EnumWindows(visit,0)
    if found:self.window=max(found)[1];break
    assert time.monotonic()<end,'owned game window did not appear';time.sleep(.1)
   self.activate();return self
  except BaseException:
   self.__exit__(None,None,None);raise
 def size(self):
  rect=W.RECT();assert self.user.GetClientRect(self.window,ctypes.byref(rect));return rect.right,rect.bottom
 def activate(self):
  self.user.ShowWindow(self.window,9)
  assert self.user.SetWindowPos(self.window,W.HWND(-1),0,0,0,0,0x1|0x10|0x40)
  try:
   width,height=self.size();point=W.POINT(width//2,height//2)
   assert self.user.ClientToScreen(self.window,ctypes.byref(point))
   end=time.monotonic()+10
   while self.user.WindowFromPoint(point)!=self.window:
    self.alive();assert time.monotonic()<end,'owned client cannot receive activation';time.sleep(.1)
   assert self.user.SetCursorPos(point.x,point.y)
   events=(self.Input*2)();events[0].data.mouse.flags=0x2;events[1].data.mouse.flags=0x4
   assert self.user.SendInput(2,events,ctypes.sizeof(self.Input))==2
  finally:assert self.user.SetWindowPos(self.window,W.HWND(-2),0,0,0,0,0x1|0x2|0x10)
  end=time.monotonic()+10
  while self.user.GetForegroundWindow()!=self.window:
   self.alive();self.user.SetForegroundWindow(self.window)
   assert time.monotonic()<end,'owned game did not receive keyboard focus';time.sleep(.1)
 def key(self,scan,down,extended=False):
  self.alive();assert self.user.GetForegroundWindow()==self.window,'refusing input outside owned game'
  event=self.Input();event.kind=1
  event.data.keyboard=self.Keyboard(0,scan,0x8|int(extended)|(0 if down else 0x2),0,0)
  assert self.user.SendInput(1,ctypes.byref(event),ctypes.sizeof(event))==1,ctypes.get_last_error()
  if down:self.pressed.add((scan,extended))
  else:self.pressed.discard((scan,extended))
 def tap(self,scan,extended=False):
  self.key(scan,True,extended);time.sleep(.06);self.key(scan,False,extended);time.sleep(.06)
 def capture(self,name):
  self.alive();width,height=self.size();assert 1<=width<=4096 and 1<=height<=4096
  dc=self.user.GetDC(self.window);memory=self.gdi.CreateCompatibleDC(dc);assert dc and memory
  info=self.Info();info.header=self.Header(ctypes.sizeof(self.Header),width,-height,1,32,0,0,0,0,0,0)
  pixels=ctypes.c_void_p();bitmap=self.gdi.CreateDIBSection(dc,ctypes.byref(info),0,ctypes.byref(pixels),None,0);assert bitmap and pixels.value
  previous=self.gdi.SelectObject(memory,bitmap)
  try:
   assert self.gdi.BitBlt(memory,0,0,width,height,dc,0,0,0x00CC0020)
   image=Image.frombytes('RGB',(width,height),ctypes.string_at(pixels,width*height*4),'raw','BGRX')
   image.save(self.proof/(self.label+'-'+name+'.png'))
   (self.proof/(self.label+'-'+name+'.json')).write_text(json.dumps({'client_pixels':[width,height],'dpi':self.user.GetDpiForWindow(self.window),'owner_pid':self.app.pid,'capture_thread_dpi':'per-monitor-v2'},indent=2)+'\n')
   return image
  finally:self.gdi.SelectObject(memory,previous);self.gdi.DeleteObject(bitmap);self.gdi.DeleteDC(memory);self.user.ReleaseDC(self.window,dc)
 def resize(self,width,height):assert self.user.SetWindowPos(self.window,None,0,0,width,height,0x2|0x4|0x10)
 def close(self):
  assert self.user.PostMessageW(self.window,0x10,0,0)
  assert self.app.wait(timeout=20)==0,self.diagnostics()
 def __exit__(self,*_):
  try:
   if self.app is not None and self.app.poll() is None:
    # Release only keys this helper pressed, even if focus changed during an
    # assertion. Always reap our own process, including on activation failure.
    for scan,extended in list(self.pressed):
     event=self.Input();event.kind=1
     event.data.keyboard=self.Keyboard(0,scan,0x8|int(extended)|0x2,0,0)
     self.user.SendInput(1,ctypes.byref(event),ctypes.sizeof(event))
    if self.window:self.user.PostMessageW(self.window,0x10,0,0)
    try:self.app.wait(timeout=10)
    except subprocess.TimeoutExpired:
     self.app.terminate()
     try:self.app.wait(timeout=10)
     except subprocess.TimeoutExpired:self.app.kill();self.app.wait(timeout=5)
  finally:
   self.log.close()
   if self.awareness:assert self.user.SetThreadDpiAwarenessContext(self.awareness)

def read_shared_file(path,maximum):
 """Snapshot an atomically replaced host file without blocking its rename."""
 kernel=ctypes.WinDLL('kernel32',use_last_error=True)
 kernel.CreateFileW.argtypes=[W.LPCWSTR,W.DWORD,W.DWORD,ctypes.c_void_p,W.DWORD,W.DWORD,W.HANDLE];kernel.CreateFileW.restype=W.HANDLE
 kernel.GetFileSizeEx.argtypes=[W.HANDLE,ctypes.POINTER(ctypes.c_longlong)];kernel.GetFileSizeEx.restype=W.BOOL
 kernel.ReadFile.argtypes=[W.HANDLE,ctypes.c_void_p,W.DWORD,ctypes.POINTER(W.DWORD),ctypes.c_void_p];kernel.ReadFile.restype=W.BOOL
 kernel.CloseHandle.argtypes=[W.HANDLE];kernel.CloseHandle.restype=W.BOOL
 handle=kernel.CreateFileW(str(path),0x80000000,7,None,3,0x80,None)
 if handle==ctypes.c_void_p(-1).value:raise ctypes.WinError(ctypes.get_last_error())
 try:
  size=ctypes.c_longlong();assert kernel.GetFileSizeEx(handle,ctypes.byref(size))
  assert 0<=size.value<=maximum,'host proof file exceeds bound'
  data=ctypes.create_string_buffer(size.value);read=W.DWORD()
  assert kernel.ReadFile(handle,data,size.value,ctypes.byref(read),None)
  assert read.value==size.value,'short host proof file'
  return data.raw
 finally:assert kernel.CloseHandle(handle)
