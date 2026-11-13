/* Extracted production loop with mocked peers and motion application. */
#include <assert.h>
#include <stdio.h>
#include <string.h>
#include <math.h>
#define NONE (-1)
#define FALSE 0
#define TRUE 1
#define MAXIMUM_TRACKED_PLAYERS 2
#define _unit_suspended_bit 24
#define TEST_FLAG(v,b) ((v)&(1u<<(b)))
typedef struct {float x,y,z;} real_point3d;
typedef struct {float i,j,k;} real_vector3d;
typedef int boolean;
typedef float real;
struct distributed_unit_state {long unit_index;real_point3d position;};
struct distributed_on_foot_bound {int initialized;};
struct unit_datum {struct {long parent_object_index;real_point3d position;real_vector3d translational_velocity;} object;struct {unsigned flags;long player_index;} unit;};
#define object_datum unit_datum
struct player_datum {int unused;};
static struct {int valid;long taken_host_time;struct distributed_unit_state state;} distributed_predictions[2];
static struct {int valid;long unit_index;} distributed_accepted[2];
static struct {long unit_index;real_vector3d velocity;float top_height;} distributed_host_speeds[2];
static struct unit_datum unit;
static struct player_datum player;
static int coop,input_enabled,live,bsp_ready,taken,bounds;
static int network_coop_active(void) {return coop;}
static int player_input_enabled(void) {return input_enabled;}
static struct player_datum *distributed_player(short i) {assert(i>=0&&i<2);return i==0?&player:NULL;}
static long distributed_living_unit(struct player_datum *p) {assert(!p||p==&player);return p&&live?7:NONE;}
static struct unit_datum *object_get(long i) {assert(i==7);return &unit;}
static struct unit_datum *unit_get(long i) {assert(i==7);return &unit;}
static int network_coop_player_has_structure_bsp(long i) {assert(i==9);return bsp_ready;}
static void distributed_on_foot_bound(short i,long u,struct distributed_on_foot_bound *b) {assert(i==0&&u==7);++bounds;b->initialized=1;distributed_host_speeds[i].unit_index=u;}
static void distributed_take_prediction(short i,long u,struct distributed_on_foot_bound const *b) {assert(i==0&&u==7&&b->initialized);++taken;}
#include "candidate.inc"
static void reset(void) {
 memset(gamepkg_script_handoffs,0,sizeof(gamepkg_script_handoffs));
 memset(distributed_predictions,0,sizeof(distributed_predictions));
 memset(distributed_accepted,0,sizeof(distributed_accepted));
 memset(distributed_host_speeds,0,sizeof(distributed_host_speeds));
 for(int i=0;i<2;i++)distributed_host_speeds[i].unit_index=NONE;
 memset(&unit,0,sizeof(unit));unit.object.parent_object_index=NONE;unit.unit.player_index=9;
 unit.object.position.z=2;unit.object.translational_velocity=(real_vector3d){1,2,3};
 distributed_predictions[0].valid=1;distributed_predictions[0].state.unit_index=7;
 distributed_predictions[0].state.position=unit.object.position;
 distributed_predictions[0].taken_host_time=123;
 distributed_accepted[0].valid=1;distributed_accepted[0].unit_index=7;
 distributed_host_speeds[0].unit_index=7;
 coop=input_enabled=live=bsp_ready=1;taken=bounds=0;
}
int main(int argc,char **argv) {
 assert(argc==2);reset();
 if(!strcmp(argv[1],"disabled")) {
  input_enabled=0;distributed_apply_predictions();
  assert(!taken&&!bounds&&!distributed_predictions[0].valid&&!distributed_accepted[0].valid);
  assert(distributed_predictions[0].taken_host_time==NONE&&distributed_host_speeds[0].unit_index==NONE);
 } else if(!strcmp(argv[1],"suspended")) {
  unit.unit.flags=1u<<24;distributed_apply_predictions();assert(!taken&&!bounds&&!distributed_accepted[0].valid);
 } else if(!strcmp(argv[1],"release")) {
  input_enabled=0;distributed_apply_predictions();assert(!taken);
  input_enabled=1;distributed_predictions[0].valid=1;distributed_apply_predictions();
  assert(taken==1&&bounds==1&&distributed_host_speeds[0].unit_index==7);
  assert(distributed_host_speeds[0].velocity.i==1&&distributed_host_speeds[0].top_height==2);
 } else if(!strcmp(argv[1],"silent_script")) {
  distributed_predictions[0].valid=0;distributed_accepted[0].valid=0;distributed_host_speeds[0].unit_index=NONE;
  input_enabled=0;distributed_apply_predictions();
  assert(gamepkg_script_handoffs[0].waiting&&!taken);
  input_enabled=1;distributed_predictions[0].valid=1;distributed_predictions[0].state.position.x=-1;
  distributed_apply_predictions();assert(!taken&&gamepkg_script_handoffs[0].waiting);
 } else if(!strcmp(argv[1],"no_unit_reset")) {
  gamepkg_script_handoffs[0].unit_index=7;gamepkg_script_handoffs[0].waiting=1;live=0;
  distributed_apply_predictions();assert(gamepkg_script_handoffs[0].unit_index==NONE&&!gamepkg_script_handoffs[0].waiting&&!taken);
 } else if(!strcmp(argv[1],"nonfinite")) {
  gamepkg_script_handoffs[0].unit_index=7;gamepkg_script_handoffs[0].waiting=1;
  distributed_predictions[0].state.position.x=NAN;distributed_apply_predictions();assert(!taken&&gamepkg_script_handoffs[0].waiting);
  distributed_predictions[0].valid=1;distributed_predictions[0].state.position.x=INFINITY;
  distributed_apply_predictions();assert(!taken&&gamepkg_script_handoffs[0].waiting);
 } else if(!strcmp(argv[1],"handoff")) {
  input_enabled=0;distributed_apply_predictions();assert(gamepkg_script_handoffs[0].waiting&&!taken);
  input_enabled=1;distributed_predictions[0].valid=1;distributed_predictions[0].state.position.x=-1;
  distributed_apply_predictions();assert(!taken&&gamepkg_script_handoffs[0].waiting&&!distributed_predictions[0].valid);
  assert(unit.object.position.x==0&&unit.object.position.z==2);
  distributed_predictions[0].valid=1;distributed_predictions[0].state.position=unit.object.position;
  distributed_apply_predictions();assert(taken==1&&!gamepkg_script_handoffs[0].waiting);
  distributed_predictions[0].valid=1;distributed_predictions[0].state.position.x=1;
  distributed_apply_predictions();assert(taken==2);
 } else if(!strcmp(argv[1],"newlife")) {
  gamepkg_script_handoffs[0].unit_index=6;gamepkg_script_handoffs[0].waiting=1;
  distributed_predictions[0].state.position.x=-1;distributed_apply_predictions();
  assert(taken==1&&!gamepkg_script_handoffs[0].waiting&&gamepkg_script_handoffs[0].unit_index==7);
 } else if(!strcmp(argv[1],"noncoop")) {
  coop=0;input_enabled=0;unit.unit.flags=1u<<24;distributed_apply_predictions();assert(taken==1&&bounds==1);
 } else if(!strcmp(argv[1],"riding")) {
  unit.object.parent_object_index=42;distributed_apply_predictions();assert(!taken&&!bounds&&!distributed_accepted[0].valid);
 } else if(!strcmp(argv[1],"dead")) {
  live=0;distributed_apply_predictions();assert(!taken&&!bounds&&!distributed_accepted[0].valid);
 } else if(!strcmp(argv[1],"bsp")) {
  bsp_ready=0;distributed_apply_predictions();assert(!taken&&bounds==1);
 } else if(!strcmp(argv[1],"oldunit")) {
  distributed_predictions[0].state.unit_index=6;distributed_apply_predictions();assert(!taken&&bounds==1);
 } else if(!strcmp(argv[1],"normal")) {
  distributed_apply_predictions();assert(taken==1&&bounds==1);
 } else {assert(!"unknown case");}
 printf("PASS %s\n",argv[1]);return 0;
}
