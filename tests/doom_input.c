/* Production input adapter tests; no window, media or physical input required. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include "../engines/doom/upstream/doomkeys.h"
#include "../engines/doom/upstream/d_event.h"
#define EXPORT(name)
static struct trace { int kind,a,b,c; } trace[4096];
static unsigned count;
static jmp_buf overflow;
static int expect_overflow;
static int quit_requested;
static unsigned quit_countdown;
static uint8_t pointer_buttons;
void I_Error(char *text,...) { (void)text;if(expect_overflow)longjmp(overflow,1);abort(); }
void D_PostEvent(event_t *event) {assert(count<4096);trace[count++]=(struct trace){2,event->data1,event->data2,event->data3};}
#include "../engines/doom/adapters/input.h"
static void pump(void) {
  int down; unsigned char key; input_dispatched=0;
  while(DG_GetKey(&down,&key)) { assert(count<4096);trace[count++]=(struct trace){1,key,down,0}; }
}
static void reset(void) {
  qr=qw=0;count=0;memset(physical_keys,0,sizeof(physical_keys));memset(action_keys,0,sizeof(action_keys));memset(delivered_keys,0,sizeof(delivered_keys));
}
int main(void) {
  assert(!engine_input(1,'a',1));assert(!engine_input(0x502,-123,321));assert(!engine_input(1,'a',0));pump();
  assert(count==3&&trace[0].kind==1&&trace[0].a=='a'&&trace[0].b==1);
  assert(trace[1].kind==2&&trace[1].a==5&&trace[1].b==-123&&trace[1].c==-321);
  assert(trace[2].kind==1&&trace[2].a=='a'&&!trace[2].b);
  reset();assert(!engine_input(1,KEY_RSHIFT,1));assert(!engine_input(1,KEY_RSHIFT,1));pump();assert(count==1);
  assert(!engine_input(1,'b',1));assert(!engine_input(3,0,0));pump();
  assert(count==3&&trace[1].kind==2&&trace[1].a==0&&trace[2].a==KEY_RSHIFT&&!trace[2].b);
  reset();event(KEY_RCTRL,1);engine_input(1,KEY_RCTRL,1);event(KEY_RCTRL,0);pump();assert(count==1&&trace[0].b==1);
  engine_input(1,KEY_RCTRL,0);pump();assert(count==2&&!trace[1].b);
  reset();const uint8_t keys[2]={'z','z'};const uint32_t masks[2]={1,2};
  action_state(keys,masks,2,1);pump();action_state(keys,masks,2,2);pump();assert(count==1&&trace[0].b);
  action_state(keys,masks,2,0);pump();assert(count==2&&!trace[1].b);
  reset();engine_input(1,'x',1);engine_input(3,0,0);pump();assert(count==1&&trace[0].kind==2);
  for(unsigned buttons=0;buttons<256;buttons++) assert(!engine_input(2|(buttons<<8),0,0));
  for(unsigned frame=0;frame<8;frame++)pump();
  assert(qr==qw);
  assert(engine_input(0x10002,0,0)==-1);assert(engine_input(2,-32769,0)==-1);assert(engine_input(2,0,32768)==-1);
  assert(engine_input(1,0,1)==-1);assert(engine_input(1,256,1)==-1);assert(engine_input(1,65,2)==-1);assert(engine_input(3,1,0)==-1);
  reset();for(unsigned i=0;i<INPUT_QUEUE;i++)engine_input(1,'x',1);
  pump();assert(qw-qr==INPUT_QUEUE-32);
  expect_overflow=1;
  if(setjmp(overflow)==0){for(unsigned i=0;i<33;i++)engine_input(1,'x',1);assert(!"queue overflow was not rejected");}
  return 0;
}
