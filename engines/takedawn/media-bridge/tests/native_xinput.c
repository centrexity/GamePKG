/* Actual Windows adapter + real Rust service; only OS device I/O is fake. */
#include <stdint.h>
#include <windows.h>
#include <xinput.h>
#include <assert.h>
#include <stdio.h>
#include <string.h>
static XINPUT_STATE physical_state[4];
static int connected[4], focused = 1;
static unsigned polls[4], rumble_calls[4];
static XINPUT_VIBRATION motors[4];
static uint64_t now_ns = 1;
static DWORD fake_get(DWORD port, XINPUT_STATE *out) {
 assert(port<4);++polls[port];
 if(!connected[port])return ERROR_DEVICE_NOT_CONNECTED;
 *out=physical_state[port];return ERROR_SUCCESS;
}
static DWORD fake_set(DWORD port, XINPUT_VIBRATION *value) {
 assert(port<4);++rumble_calls[port];motors[port]=*value;
 return connected[port]?ERROR_SUCCESS:ERROR_DEVICE_NOT_CONNECTED;
}
static uint64_t fake_now(void){return now_ns;}
static int fake_focus(void){return focused;}
#define GAMEPKG_XINPUT_GET_STATE fake_get
#define GAMEPKG_XINPUT_SET_STATE fake_set
#define GAMEPKG_XINPUT_TIME_NS fake_now
#define GAMEPKG_XINPUT_FOCUSED fake_focus
#include "native_xinput.h"
static void advance(void){now_ns+=UINT64_C(300000000);}
static unsigned total_polls(void){return polls[0]+polls[1]+polls[2]+polls[3];}
int main(void) {
 uint64_t clock=gamepkg_input_time_ns();assert(clock>0&&gamepkg_input_time_ns()>=clock);
 XINPUT_STATE state;
 connected[3]=1;
 assert(gamepkg_xinput_get_state(0,&state)==ERROR_SUCCESS);
 assert(state.Gamepad.wButtons==0&&state.Gamepad.sThumbLX==0);
 /* One physical controller gives exactly one neutral presence slot. */
 assert(gamepkg_xinput_get_state(1,&state)==ERROR_DEVICE_NOT_CONNECTED);
 assert(gamepkg_xinput_get_state(2,&state)==ERROR_DEVICE_NOT_CONNECTED);
 assert(gamepkg_xinput_get_state(3,&state)==ERROR_DEVICE_NOT_CONNECTED);
 assert(total_polls()==4);
 physical_state[3].Gamepad.wButtons=XINPUT_GAMEPAD_A;
 physical_state[3].Gamepad.sThumbLX=-32768;
 physical_state[3].Gamepad.bRightTrigger=255;
 assert(gamepkg_xinput_get_state(0,&state)==ERROR_SUCCESS);
 assert(state.Gamepad.wButtons==XINPUT_GAMEPAD_A&&state.Gamepad.sThumbLX==-32768&&state.Gamepad.bRightTrigger==255);
 assert(total_polls()==5); /* Repeated logical0 reads are never throttled. */
 DWORD packet=state.dwPacketNumber;
 assert(gamepkg_xinput_get_state(0,&state)==ERROR_SUCCESS&&state.dwPacketNumber==packet);
 XINPUT_VIBRATION vibration={12345,54321};
 assert(gamepkg_xinput_set_state(0,&vibration)==ERROR_SUCCESS);
 assert(rumble_calls[3]==1&&motors[3].wLeftMotorSpeed==12345&&motors[3].wRightMotorSpeed==54321);
 assert(!rumble_calls[0]);
 /* Whole input domain round-trips through the real core and adapter. */
 unsigned before=total_polls();
 for(unsigned raw=0;raw<=65535;raw++) {
  XINPUT_GAMEPAD *pad=&physical_state[3].Gamepad;
  pad->wButtons=(WORD)raw;pad->sThumbLX=(SHORT)raw;pad->sThumbLY=(SHORT)(raw^0x8000u);
  pad->sThumbRX=(SHORT)(raw+1);pad->sThumbRY=(SHORT)(raw-1);
  pad->bLeftTrigger=(BYTE)raw;pad->bRightTrigger=(BYTE)(raw>>8);
  physical_state[3].dwPacketNumber=raw+100;
  now_ns+=100;
  assert(gamepkg_xinput_get_state(0,&state)==ERROR_SUCCESS);
  assert(!memcmp(pad,&state.Gamepad,sizeof *pad));
 }
 assert(total_polls()-before==65536); /* No 4x poll amplification. */
 /* Focus changes make a new packet even if hardware's packet is unchanged. */
 physical_state[3].Gamepad.wButtons=XINPUT_GAMEPAD_A;
 physical_state[3].Gamepad.sThumbLX=100;
 assert(gamepkg_xinput_get_state(0,&state)==ERROR_SUCCESS);packet=state.dwPacketNumber;
 focused=0;
 assert(gamepkg_xinput_get_state(0,&state)==ERROR_SUCCESS);
 assert(state.Gamepad.wButtons==0&&state.Gamepad.sThumbLX==0&&state.dwPacketNumber!=packet);
 focused=1;
 assert(gamepkg_xinput_get_state(0,&state)==ERROR_SUCCESS&&state.Gamepad.wButtons==0);
 physical_state[3].Gamepad.wButtons=0;gamepkg_xinput_get_state(0,&state);
 physical_state[3].Gamepad.wButtons=XINPUT_GAMEPAD_A;
 assert(gamepkg_xinput_get_state(0,&state)==ERROR_SUCCESS&&state.Gamepad.wButtons==XINPUT_GAMEPAD_A);
 /* A second physical slot can join player1 without replacing player0. */
 connected[0]=1;advance();
 assert(gamepkg_xinput_get_state(1,&state)==ERROR_SUCCESS&&state.Gamepad.wButtons==0);
 assert(gamepkg_xinput_get_state(2,&state)==ERROR_DEVICE_NOT_CONNECTED);
 physical_state[0].Gamepad.wButtons=XINPUT_GAMEPAD_B;advance();
 assert(gamepkg_xinput_get_state(1,&state)==ERROR_SUCCESS&&state.Gamepad.wButtons==XINPUT_GAMEPAD_B);
 vibration.wLeftMotorSpeed=2222;
 assert(gamepkg_xinput_set_state(1,&vibration)==ERROR_SUCCESS&&motors[0].wLeftMotorSpeed==2222);
 connected[3]=0;advance();
 assert(gamepkg_xinput_get_state(0,&state)==ERROR_DEVICE_NOT_CONNECTED);
 assert(gamepkg_xinput_get_state(1,&state)==ERROR_SUCCESS&&state.Gamepad.wButtons==XINPUT_GAMEPAD_B);
 connected[3]=1;physical_state[3].Gamepad.wButtons=0;advance();
 assert(gamepkg_xinput_get_state(0,&state)==ERROR_SUCCESS&&state.Gamepad.wButtons==0);
 physical_state[3].Gamepad.wButtons=XINPUT_GAMEPAD_X;
 assert(gamepkg_xinput_get_state(0,&state)==ERROR_SUCCESS&&state.Gamepad.wButtons==XINPUT_GAMEPAD_X);
 assert(gamepkg_xinput_get_state(4,&state)==ERROR_DEVICE_NOT_CONNECTED);
 assert(gamepkg_xinput_get_state(0,NULL)==ERROR_BAD_ARGUMENTS);
 gamepkg_xinput_shutdown();
 assert(motors[0].wLeftMotorSpeed==0&&motors[3].wRightMotorSpeed==0);
 gamepkg_input_gamepad shared;
 assert(gamepkg_input_gamepad_state(0,&shared)==0&&gamepkg_input_gamepad_state(1,&shared)==0);
 printf("PASS native Windows XInput adapter %zu-bit: first press, physical slots, 65536 raw reports, cadence/backoff, focus packets, hotplug, rumble routing and shutdown\n",sizeof(void*)*8);
 return 0;
}
