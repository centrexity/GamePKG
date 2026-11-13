/* Actual budget and game_time_update, external engine APIs mocked. */
#include <assert.h>
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
typedef float real;
typedef int boolean;
typedef uint32_t halo_u32;
#define FALSE 0
#define TRUE 1
#define NONE (-1)
#define TICKS_PER_SECOND 30
#define MAXIMUM_PING_MILLISECONDS 9999
#define SOME_LARGE_NUMBER_OF_TICKS 1000
#define MIN(a,b) ((a)<(b)?(a):(b))
#define match_assert(...) ((void)0)
#define profile_scope(name,...) __VA_ARGS__
enum { _game_connection_local, _game_connection_network_client, _game_connection_network_server, _game_connection_film_playback };
static struct {int active;long local_time,server_time;short last_local_time_elapsed;float speed,leftover_dt;} state;
static __typeof__(state) *game_time_globals=&state;
static long distributed_host_time;
static halo_u32 distributed_host_time_received,now_ms;
static int distributed_own_ping_valid;
static float distributed_own_ping_milliseconds;
static int connection,menu,fixed,held;
static int ticks,frames,network_ticks,render_ticks;
static long game_connection(void) {return connection;}
static long game_time_get(void) {return state.local_time;}
static halo_u32 system_milliseconds(void) {return now_ms;}
static int game_time_menu_scene(void) {return menu;}
static int game_time_held(void) {return held;}
static int halo_fixed_step(void) {return fixed;}
static float game_time_get_speed(void) {return state.speed;}
static void update_client_local_ticks(long n) {(void)n;}
static void *global_network_game_server_get(void) {return 0;}
static void network_game_server_update_ticks(void *server,short n) {(void)server;(void)n;}
static long update_client_get_maximum_possible_server_time(void) {return LONG_MAX;}
static void game_tick(void) {++ticks;}
static void render_interpolation_tick(void) {++render_ticks;}
static void network_distributed_tick(void) {++network_ticks;}
static void render_interpolation_frame_begin(void) {}
static void render_interpolation_frame_end(void) {}
static void game_frame(float dt) {assert(dt>=0);++frames;}
#include "candidate.inc"
static void reset(void) {
 memset(&state,0,sizeof(state));state.active=1;state.speed=1;
 connection=_game_connection_network_client;menu=fixed=held=0;
 distributed_host_time=100;state.local_time=state.server_time=100;
 distributed_host_time_received=now_ms=1000;
 distributed_own_ping_valid=0;distributed_own_ping_milliseconds=0;
 gamepkg_clock_slew_valid=FALSE;gamepkg_clock_slew_tick=NONE;
 ticks=frames=network_ticks=render_ticks=0;
}
int main(int argc,char **argv) {
 assert(argc==2);reset();
 if(!strcmp(argv[1],"ahead")) {
  state.local_time=110;assert(gamepkg_distributed_client_ticks(30)==0);
 } else if(!strcmp(argv[1],"window")) {
  assert(gamepkg_distributed_client_ticks(30)==2);
  state.local_time=101;assert(gamepkg_distributed_client_ticks(30)==1);
  state.local_time=95;assert(gamepkg_distributed_client_ticks(30)==7);
 } else if(!strcmp(argv[1],"age_ping")) {
  now_ms=1500;distributed_own_ping_valid=1;distributed_own_ping_milliseconds=200;
  assert(gamepkg_distributed_client_ticks(30)==20);
 } else if(!strcmp(argv[1],"invalid_ping")) {
  float bad[]={NAN,INFINITY,-1,10000};distributed_own_ping_valid=1;
  for(unsigned i=0;i<sizeof(bad)/sizeof(*bad);i++) {distributed_own_ping_milliseconds=bad[i];assert(gamepkg_distributed_client_ticks(30)==2);}
 } else if(!strcmp(argv[1],"wrap")) {
  distributed_host_time_received=0xfffffff0u;now_ms=50;assert(gamepkg_distributed_client_ticks(30)==3);
 } else if(!strcmp(argv[1],"bounds")) {
  assert(gamepkg_distributed_client_ticks(0)==0&&gamepkg_distributed_client_ticks(-1)==0);
  distributed_host_time=0x3fffffff;now_ms=0xffffffffu;distributed_host_time_received=0;
  state.local_time=INT_MAX-7;assert(gamepkg_distributed_client_ticks(30)==0);
  distributed_host_time=100;state.local_time=95;now_ms=0;assert(gamepkg_distributed_client_ticks(LONG_MAX)==7);
 } else if(!strcmp(argv[1],"notclient")) {
  connection=_game_connection_network_server;state.local_time=1000;assert(gamepkg_distributed_client_ticks(30)==30);
  connection=_game_connection_local;assert(gamepkg_distributed_client_ticks(30)==30);
  connection=_game_connection_network_client;distributed_host_time=NONE;assert(gamepkg_distributed_client_ticks(30)==30);
 } else if(!strcmp(argv[1],"normal60")||!strcmp(argv[1],"normal120")) {
  int fps=!strcmp(argv[1],"normal60")?60:120;
  for(int i=0;i<fps;i++) {distributed_host_time=100+(i*30)/fps;now_ms=1000+(i*1000)/fps;distributed_host_time_received=now_ms;game_time_update(1.0f/fps);}
  assert(ticks>=29&&ticks<=30&&render_ticks==ticks&&network_ticks==ticks&&frames==fps);
  assert(state.local_time==100+ticks&&state.server_time==state.local_time);
 } else if(!strcmp(argv[1],"partial")) {
  state.local_time=state.server_time=101;game_time_update(.1f);
  assert(ticks==1&&network_ticks==1&&render_ticks==1&&state.local_time==102&&state.last_local_time_elapsed==1);
 } else if(!strcmp(argv[1],"held_start")) {
  held=1;game_time_update(.1f);assert(!ticks&&!network_ticks&&state.local_time==100&&state.last_local_time_elapsed==0&&frames==1);
 } else if(!strcmp(argv[1],"hold_release")) {
  state.local_time=state.server_time=150;game_time_update(.1f);
  assert(!ticks&&state.last_local_time_elapsed==0&&state.leftover_dt==0&&state.local_time==150&&frames==1);
  distributed_host_time=151;game_time_update(.1f);
  assert(ticks==3&&state.local_time==153&&state.last_local_time_elapsed==3);
 } else if(!strcmp(argv[1],"menu")) {
  menu=1;state.local_time=state.server_time=150;game_time_update(.1f);assert(ticks==3&&!network_ticks);
 } else if(!strcmp(argv[1],"host")) {
  connection=_game_connection_network_server;state.local_time=state.server_time=150;game_time_update(.1f);assert(ticks==3&&network_ticks==3);
 } else if(!strcmp(argv[1],"fixedlocal")) {
  connection=_game_connection_local;fixed=1;game_time_update(.001f);assert(ticks==1&&state.leftover_dt==0);
 } else if(!strcmp(argv[1],"inactive")) {
  state.active=0;state.last_local_time_elapsed=7;game_time_update(.1f);assert(!ticks&&!frames&&state.last_local_time_elapsed==0);
 } else if(!strcmp(argv[1],"properties")) {
  uint32_t random=0x51a713;
  for(int i=0;i<10000;i++) {
   random=random*1664525u+1013904223u;distributed_host_time=10000+(random%10000);
   random=random*1664525u+1013904223u;state.local_time=distributed_host_time+(long)(random%201)-100;
   random=random*1664525u+1013904223u;now_ms=random;distributed_host_time_received=now_ms-random%5000;
   long request=1+random%30,original=state.local_time;long allowed=gamepkg_distributed_client_ticks(request);
   assert(allowed>=0&&allowed<=request&&state.local_time==original);
  }
 } else if(!strcmp(argv[1],"catchup60")||!strcmp(argv[1],"catchup120")) {
  int fps=!strcmp(argv[1],"catchup60")?60:120;
  state.local_time=state.server_time=88;distributed_own_ping_valid=1;
  for(int i=0;i<fps*5;i++) {
   long before=state.local_time;distributed_host_time=100+(i*30)/fps;
   now_ms=1000+(i*1000)/fps;distributed_host_time_received=now_ms;
   game_time_update(1.0f/fps);
   assert(state.local_time>=before&&state.local_time-before<=2);
   assert(state.local_time<=distributed_host_time+2);
  }
  assert(state.local_time>=distributed_host_time-2);
  assert(ticks==network_ticks&&ticks==render_ticks&&state.local_time==88+ticks);
 } else if(!strcmp(argv[1],"catchup_rate")) {
  state.local_time=50;distributed_own_ping_valid=1;
  assert(gamepkg_distributed_client_ticks(1)==2);
  for(int i=0;i<100;i++)assert(gamepkg_distributed_client_ticks(1)==1);
  now_ms=1249;distributed_host_time_received=now_ms;assert(gamepkg_distributed_client_ticks(1)==1);
  now_ms=1250;distributed_host_time_received=now_ms;assert(gamepkg_distributed_client_ticks(1)==2);
  assert(gamepkg_distributed_client_ticks(30)==30);
 } else if(!strcmp(argv[1],"catchup_zero")) {
  state.local_time=50;distributed_own_ping_valid=1;
  assert(gamepkg_distributed_client_ticks(0)==0&&gamepkg_distributed_client_ticks(-1)==0);
  assert(!gamepkg_clock_slew_valid);
 } else if(!strcmp(argv[1],"catchup_stale")) {
  state.local_time=50;distributed_own_ping_valid=1;
  now_ms=2001;assert(gamepkg_distributed_client_ticks(1)==1);
  now_ms=2000;assert(gamepkg_distributed_client_ticks(1)==2);
 } else if(!strcmp(argv[1],"catchup_invalid_ping")) {
  float bad[]={NAN,INFINITY,-1,10000};state.local_time=50;distributed_own_ping_valid=1;
  for(unsigned i=0;i<sizeof(bad)/sizeof(*bad);i++){distributed_own_ping_milliseconds=bad[i];assert(gamepkg_distributed_client_ticks(1)==1);}
  distributed_own_ping_valid=0;distributed_own_ping_milliseconds=0;assert(gamepkg_distributed_client_ticks(1)==1);
 } else if(!strcmp(argv[1],"catchup_wrap")) {
  state.local_time=50;distributed_own_ping_valid=1;now_ms=0xfffffff0u;distributed_host_time_received=now_ms;
  assert(gamepkg_distributed_client_ticks(1)==2);
  now_ms=100;distributed_host_time_received=now_ms;assert(gamepkg_distributed_client_ticks(1)==1);
  now_ms=300;distributed_host_time_received=now_ms;assert(gamepkg_distributed_client_ticks(1)==2);
 } else if(!strcmp(argv[1],"catchup_epoch")) {
  state.local_time=200;distributed_host_time=250;distributed_own_ping_valid=1;assert(gamepkg_distributed_client_ticks(1)==2);
  state.local_time=0;distributed_host_time=100;now_ms=1100;distributed_host_time_received=now_ms;assert(gamepkg_distributed_client_ticks(1)==2);
  distributed_host_time=NONE;assert(gamepkg_distributed_client_ticks(1)==1&&!gamepkg_clock_slew_valid);
 } else if(!strcmp(argv[1],"catchup_deadband")) {
  distributed_own_ping_valid=1;assert(gamepkg_distributed_client_ticks(1)==1);
  state.local_time=99;assert(gamepkg_distributed_client_ticks(1)==1);
  state.local_time=98;assert(gamepkg_distributed_client_ticks(1)==2);
 } else {assert(!"unknown case");}
 printf("PASS %s\n",argv[1]);return 0;
}
