/* Ordered desktop/controller input for DOOM. GPL-2.0-or-later. */
#define INPUT_QUEUE 1024
static struct input_event { uint32_t kind; int32_t a,b; } input_queue[INPUT_QUEUE];
static unsigned qr,qw, input_dispatched;
static uint8_t physical_keys[256], action_keys[256], delivered_keys[256];
static void queue_input(uint32_t kind,int32_t a,int32_t b) {
  if(qw-qr>=INPUT_QUEUE) I_Error("DOOM input queue exceeded its bound");
  input_queue[qw++ % INPUT_QUEUE]=(struct input_event){kind,a,b};
}
static void set_key(uint8_t *source,unsigned key,int down,int repeat) {
  if(!key || key>255) return;
  int was=physical_keys[key]||action_keys[key];
  int repeated=source[key]&&down&&repeat&&key!=KEY_RSHIFT&&key!=KEY_RCTRL&&key!=KEY_RALT;
  source[key]=(uint8_t)!!down;
  int now=physical_keys[key]||action_keys[key];
  if(was!=now || repeated) queue_input(1,(int32_t)key,now);
}
static void event(int key,int down) { set_key(action_keys,(unsigned)key,down,0); }
static void action_state(const uint8_t *keys,const uint32_t *masks,unsigned length,uint32_t now) {
  uint8_t desired[256]={0};
  for(unsigned i=0;i<length;i++) if(now&masks[i]) desired[keys[i]]=1;
  for(unsigned key=1;key<256;key++) event((int)key,desired[key]);
}
EXPORT("engine_input") int engine_input(int kind,int a,int b) {
  if(kind==1) {
    if(a<=0||a>255||(b!=0&&b!=1)) return -1;
    if(quit_requested==1&&b) quit_requested=2;
    if(quit_requested||quit_countdown) return 0;
    set_key(physical_keys,(unsigned)a,b,1); return 0;
  }
  if((kind&255)==2 && (unsigned)kind<=0xff02 && a>=-32768 && a<=32767 && b>=-32768 && b<=32767) {
    uint8_t next=(uint8_t)((unsigned)kind>>8);
    if(quit_requested==1 && (next&~pointer_buttons)) quit_requested=2;
    pointer_buttons=next;
    if(quit_requested||quit_countdown) return 0;
    queue_input((uint32_t)kind,a,b); return 0;
  }
  if(kind==3 && !a && !b) {
    pointer_buttons=0;
    qr=qw=0;
    memset(physical_keys,0,sizeof(physical_keys));
    queue_input(2,0,0);
    for(unsigned key=1;key<256;key++) if(delivered_keys[key]&&!action_keys[key]) queue_input(1,(int32_t)key,0);
    return 0;
  }
  return -1;
}
int DG_GetKey(int *pressed,unsigned char *key) {
  /* d_event.c has a 64-entry ring shared with other sources. Never wrap it
   * before D_ProcessEvents runs; pending events retain their original order. */
  while(qr!=qw && input_dispatched<32) {
    struct input_event next=input_queue[qr++ % INPUT_QUEUE]; input_dispatched++;
    if(next.kind==1) { *key=(unsigned char)next.a; *pressed=next.b; delivered_keys[next.a]=(uint8_t)!!next.b; return 1; }
    event_t mouse={.type=ev_mouse,.data1=(int)(next.kind>>8),.data2=next.a,.data3=-next.b};
    D_PostEvent(&mouse);
  }
  return 0;
}
