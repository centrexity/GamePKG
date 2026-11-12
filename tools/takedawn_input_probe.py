"""Maintainer-only normal-pad and framebuffer probe for owned Takedawn sessions.

No game-state/AI writes or outcome injection. Players do not need this tool.
Each Bridge controls one selected player; the original runtime applies layouts.
"""
from pathlib import Path
import hashlib,json,os,re,time

def controls(text):
 state={}
 for line in text.splitlines():
  parts=line.split()
  if len(parts)==6 and parts[0]=='S' and parts[4]=='->':state[parts[2]]=(int(parts[5],16),int(parts[1]))
 return state

def pad_sequence(command,initial=False,player=1,scheme="xbox"):
 if type(player) is not int or player not in [1,2]:raise ValueError("player must be 1 or 2")
 if scheme not in ["xbox","ps2"]:raise ValueError("unsupported physical control layout")
 prefix="" if player==1 else "p2-"
 allowed={'action','id','steer','steer_ms','drive_ms','throttle','boost'}
 if set(command)-allowed or command.get('action')!='step':raise ValueError('invalid step keys')
 steer=command.get('steer','none');turn=command.get('steer_ms',0);duration=command.get('drive_ms',600);throttle=command.get('throttle',200);boost=command.get('boost',False)
 if steer not in ['none','left','right'] or any(type(v) is not int for v in [turn,duration,throttle]) or type(boost) is not bool:raise ValueError('invalid control types')
 if not 100<=duration<=1200 or not 0<=turn<=min(duration,400) or not 0<=throttle<=255 or steer=='none' and turn:raise ValueError('invalid control bounds')
 pedal=('a=' if scheme=='ps2' else 'rt=')+str(throttle)
 if boost:pedal+='+'+('black' if scheme=='ps2' else 'a')
 result=[] if initial else ['start:150']
 if turn:result.append(pedal+'+l'+steer+':'+str(turn))
 if duration>turn:result.append(pedal+':'+str(duration-turn))
 result.append('start:150');return [prefix+step for step in result]

def capture_wait_seconds(remaining_frames):
 if type(remaining_frames) is not int or not 0<=remaining_frames<=3600:raise ValueError('invalid private frame observation span')
 return max(15,remaining_frames/30+5)

class Bridge:
 def __init__(self,work,process,player=1,scheme="xbox"):
  pad_sequence({'action':'step'},player=player,scheme=scheme)
  self.work=Path(work);self.run=self.work/'run';self.process=process;self.generation=0
  self.player=player;self.scheme=scheme;self.phase_key=f'p{player}_phase';self.prefix='' if player==1 else 'p2-'
 def alive(self):
  if self.process.poll() is not None:raise RuntimeError('owned game driver exited: '+str(self.process.returncode))
 def text(self,name):
  p=self.run/name;return p.read_text(errors='replace') if p.exists() else ''
 def state(self):return controls(self.text('controls.txt'))
 def flip(self):
  observed=[]
  for name in ['log.txt','events.txt']:
   p=self.run/name
   if not p.exists():continue
   with p.open('rb') as f:f.seek(max(0,p.stat().st_size-16384));text=f.read().decode(errors='replace')
   observed.extend(map(int,re.findall(r'\[GPU\] flip (\d+)',text)))
  return max(observed,default=0)
 def wait(self,condition,seconds):
  until=time.monotonic()+seconds
  while True:
   self.alive();value=condition()
   if value:return value
   if time.monotonic()>=until:raise TimeoutError('normal input/capture observation deadline')
   time.sleep(.025)
 def append(self,sequence):
  with (self.run/'pad.txt').open('a') as f:f.write('\n'.join(sequence)+'\n')
 def capture_plan(self):
  self.generation+=1;first=self.flip();frames=[first+i for i in range(6,91,6)]
  temporary=self.run/'dumpat-next.txt';temporary.write_text(' '*self.generation+'\n'.join(map(str,frames))+'\n');os.replace(temporary,self.run/'dumpat.txt');return frames
 def picture(self,frames,before=None):
  from PIL import Image
  self.wait(lambda:self.flip()>frames[-1],capture_wait_seconds(max(0,frames[-1]-self.flip())))
  candidates=[n for n in frames if (before is None or n<before-1) and (self.run/f'fb_flip_{n:05d}.bmp').is_file()]
  if not candidates:raise RuntimeError('no actual unpaused framebuffer available')
  chosen=max(candidates);source=self.run/f'fb_flip_{chosen:05d}.bmp';folder=self.run/'normal-input-frames';folder.mkdir(exist_ok=True);output=folder/f'step-{self.generation:03d}-flip-{chosen:05d}.png';Image.open(source).save(output)
  for n in frames:
   p=self.run/f'fb_flip_{n:05d}.bmp'
   if p.exists():p.unlink()
  return {'frame':chosen,'image':str(output),'sha256':hashlib.sha256(output.read_bytes()).hexdigest()}
 def step(self,command,initial=False):
  state=self.state();paused=state.get('paused_by',(None,0))
  if state.get(self.phase_key,(None,0))[0]!=2 or (not initial and paused[0]!=self.player-1):raise RuntimeError('step requires original in-play/pause state')
  sequence=pad_sequence(command,initial,self.player,self.scheme);executed=[]
  if not initial:
   self.wait(lambda:self.flip()>paused[1]+90,10)
   self.append([self.prefix+'start:150']);executed.append(self.prefix+'start:150')
   self.wait(lambda:self.state().get('paused_by',(None,0))[0]==0xffffffff,10)
  before=max(self.flip(),self.state().get('paused_by',(None,0))[1]);frames=self.capture_plan()
  driving=sequence if initial else sequence[1:]
  self.append(driving);executed.extend(driving)
  def ended():
   current=self.state();phase=current.get(self.phase_key,(0,0));pause=current.get('paused_by',(None,0))
   if phase[0]==3:return 'result',phase[1]
   if pause[0]==self.player-1 and pause[1]>before:return 'paused',pause[1]
   return None
  mode,flip=self.wait(ended,15)
  if mode=='paused':self.wait(lambda:self.flip()>flip+90,10)
  picture=self.picture(frames,flip if mode=='paused' else None)
  return {'mode':mode,'phase':self.state().get(self.phase_key),'paused':self.state().get('paused_by'),'pad_sequence':executed,**picture}
 def button(self,name):
  if name not in ['a','b','up','down','left','right','start','y']:raise ValueError('unsupported normal menu button')
  self.append([self.prefix+name+':400']);first=self.flip();self.wait(lambda:self.flip()>first+180,15);frames=self.capture_plan();return {'mode':'menu','button':name,**self.picture(frames)}
