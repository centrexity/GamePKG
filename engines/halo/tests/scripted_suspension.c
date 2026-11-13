/* SPDX-License-Identifier: Apache-2.0
 * Asset-free harness around the actual upstream snapshot codec and setter. */
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
typedef uint8_t byte;
typedef uint16_t word;
typedef int32_t halo_s32;
typedef int boolean;
typedef struct { float x,y,z; } real_point3d;
typedef struct { float i,j,k; } real_vector3d;
#define TRUE 1
#define FALSE 0
#define NONE (-1)
#define NO_PLAYER 255
#define FLAG(bit) (1u << (bit))
#define TEST_FLAG(flags,bit) (((flags)&FLAG(bit))!=0)
#define SET_FLAG(flags,bit,value) ((value)?((flags)|=FLAG(bit)):((flags)&=~FLAG(bit)))
#define csmemcpy memcpy
#define csmemset memset
#define DISTRIBUTED_UNIT_SCALE 32767.0f
#define _object_type_biped 0
#define _biped_airborne_bit 0
typedef float real;
struct unit_datum { struct { int type; long parent_object_index; real_vector3d translational_velocity; } object; struct { uint32_t flags; real_vector3d desired_facing_vector,desired_aiming_vector,aiming_vector,aiming_velocity,desired_looking_vector,looking_vector,looking_velocity; } unit; };
struct player_datum { long local_player_index; long identifier; };
#define DATUM_INDEX_NEW(index,identifier) ((long)(index)+(long)(identifier))
static unsigned facing_calls;
static real_vector3d last_facing;
static real dot_product3d(const real_vector3d *a,const real_vector3d *b) {return a->i*b->i+a->j*b->j+a->k*b->k;}
static void normalize3d(real_vector3d *v) {real length=sqrtf(dot_product3d(v,v));v->i/=length;v->j/=length;v->k/=length;}
static void distributed_vector_unpack(const void *packed,real scale,real_vector3d *out) {int16_t p[3];memcpy(p,packed,sizeof(p));out->i=p[0]/scale;out->j=p[1]/scale;out->k=p[2]/scale;}
static void gamepkg_scripted_view_local_facing(long player,const real_vector3d *world,real_vector3d *local) {(void)player;(void)world;(void)local;assert(!"Parent basis covered by separate actual-engine fixture");}
static void player_control_set_facing(long player,const real_vector3d *v) {assert(player==0);++facing_calls;last_facing=*v;}

struct biped_datum { struct { uint32_t flags; } biped; };
static struct unit_datum fixture;
static struct biped_datum biped;
static real_vector3d zero;
static real_vector3d *global_zero_vector3d=&zero;
static unsigned setter_calls,unit_queries;
static boolean coop;
static boolean network_coop_active(void) { return coop; }
static struct unit_datum *unit_get(long index) { assert(index==7);++unit_queries;return &fixture; }
static struct biped_datum *biped_get(long index) { assert(index==7);return &biped; }
static void coop_scripts_suspend_followers(long index,boolean suspended) { assert(index==7);(void)suspended;++setter_calls; }
#include "reviewed_snapshot.inc"

static void transitions(void)
{
    struct distributed_unit_state state={0};
    fixture.unit.flags=FLAG(_unit_active_camouflaged_bit)|FLAG(_unit_controllable_bit);
    fixture.object.type=_object_type_biped;
    fixture.object.translational_velocity=(real_vector3d){3,4,5};
    biped.biped.flags=FLAG(_biped_airborne_bit);
    state.unit_flags=FLAG(_distributed_unit_suspended_bit);
    coop=FALSE;distributed_apply_scripted_suspension(-999,&state);
    assert(!unit_queries&&!setter_calls);
    assert(fixture.object.translational_velocity.i==3);
    coop=TRUE;state.unit_flags=0;distributed_apply_scripted_suspension(7,&state);
    assert(!setter_calls&&fixture.object.translational_velocity.i==3);
    state.unit_flags=FLAG(_distributed_unit_suspended_bit);
    distributed_apply_scripted_suspension(7,&state);
    assert(setter_calls==1&&TEST_FLAG(fixture.unit.flags,_unit_suspended_bit));
    assert(fixture.object.translational_velocity.i==0&&fixture.object.translational_velocity.j==0&&fixture.object.translational_velocity.k==0);
    assert(!TEST_FLAG(biped.biped.flags,_biped_airborne_bit));
    assert(TEST_FLAG(fixture.unit.flags,_unit_active_camouflaged_bit)&&TEST_FLAG(fixture.unit.flags,_unit_controllable_bit));
    fixture.object.translational_velocity.i=9;biped.biped.flags|=FLAG(_biped_airborne_bit);
    distributed_apply_scripted_suspension(7,&state);
    assert(setter_calls==1&&fixture.object.translational_velocity.i==9&&TEST_FLAG(biped.biped.flags,_biped_airborne_bit));
    state.unit_flags=0;distributed_apply_scripted_suspension(7,&state);
    assert(setter_calls==2&&!TEST_FLAG(fixture.unit.flags,_unit_suspended_bit));
    assert(fixture.object.translational_velocity.i==0&&!TEST_FLAG(biped.biped.flags,_biped_airborne_bit));
    distributed_apply_scripted_suspension(7,&state);assert(setter_calls==2);
    coop=FALSE;state.unit_flags=255;distributed_apply_scripted_suspension(-999,&state);assert(setter_calls==2);
    puts("PASS transition-only original setter, velocity/airborne effects, unrelated flags, non-coop isolation");
}
static void snapshots(void)
{
    unsigned cases=0,truncations=0;
    for(unsigned flags=0;flags<256;flags++)for(unsigned own=0;own<2;own++)for(unsigned shape=0;shape<5;shape++)for(unsigned view=0;view<3;view++) {
        struct distributed_unit_state state={0},decoded;
        struct { byte before[16],data[DISTRIBUTED_UNIT_STATE_MAXIMUM_SIZE],after[16]; } wire;
        memset(&wire,0xa5,sizeof(wire));
        state.player_index=17;state.unit_index=0x12345678;state.vehicle_index=NONE;state.seat_index=NONE;state.killing_player_index=NO_PLAYER;
        state.up.k=32767;state.unit_flags=(byte)flags;
        if(shape<3) {
            state.flags=FLAG(_distributed_unit_alive_bit)|FLAG(_distributed_unit_predicted_bit);
            state.predicted_time=-32760;state.body_vitality=16384;state.shield_vitality=8192;
            if(shape!=1)state.flags|=FLAG(_distributed_unit_placed_bit);
            state.position=(real_point3d){1.25f,-2.5f,3.75f};state.velocity=(struct distributed_vector){-300,400,500};
            state.forward.i=32767;
            if(shape==1){state.vehicle_index=0x23456789;state.seat_index=3;}
            if(shape==2){state.vehicle_index=0x23456789;state.seat_index=2;state.up.i=200;state.current_body_damage=23;state.recent_body_damage=42;state.current_shield_damage=9;state.recent_shield_damage=3;state.powerup_durations[0]=300;state.powerup_durations[1]=-1;state.active_camouflage=255;}
        } else if(shape==3){state.killing_player_index=5;state.flags=FLAG(_distributed_unit_friendly_fire_bit);}
        state.scripted_view=view!=0;state.view_for_owner=view==2;
        unsigned expected_view=view!=0&&(!own||view==2);
        for(unsigned v=0;v<7;v++)state.scripted_vectors[v]=(struct distributed_vector){(short)(flags+v), (short)(32767-flags), (short)(-32768+flags)};
        word length=distributed_unit_state_write(&state,own,wire.data);
        assert(length>=DISTRIBUTED_UNIT_STATE_MINIMUM_SIZE&&length<=DISTRIBUTED_UNIT_STATE_MAXIMUM_SIZE);
        for(unsigned i=0;i<16;i++)assert(wire.before[i]==0xa5&&wire.after[i]==0xa5);
        for(unsigned i=length;i<sizeof(wire.data);i++)assert(wire.data[i]==0xa5);
        assert(distributed_unit_state_read(wire.data,wire.data+length,&decoded)==length);
        if(shape<3) {
            assert(decoded.unit_flags==flags);
            assert(decoded.scripted_view==expected_view);
            assert(decoded.view_for_owner==expected_view);
            if(expected_view)assert(!memcmp(decoded.scripted_vectors,state.scripted_vectors,sizeof(state.scripted_vectors)));
        }
        else assert(decoded.unit_flags==0);
        byte canonical[DISTRIBUTED_UNIT_STATE_MAXIMUM_SIZE];
        assert(distributed_unit_state_write(&decoded,TRUE,canonical)==length);
        assert(!memcmp(wire.data,canonical,length));
        for(unsigned cut=0;cut<length;cut++) {
            byte *short_packet=malloc(cut?cut:1);assert(short_packet);
            memcpy(short_packet,wire.data,cut);
            assert(!distributed_unit_state_read(short_packet,short_packet+cut,&decoded));
            free(short_packet);++truncations;
        }
        ++cases;
    }
    printf("PASS %u snapshot flag/shape/ownership round-trips; %u exact-length truncated allocations rejected\n",cases,truncations);
}

static void application(void)
{
 unsigned cases=0;
 for(unsigned enabled=0;enabled<2;enabled++)for(unsigned own=0;own<2;own++)for(unsigned scripted=0;scripted<2;scripted++) {
  struct distributed_unit_state state={0},decoded;
  struct player_datum player={own?0:NONE,0};
  byte wire[DISTRIBUTED_UNIT_STATE_MAXIMUM_SIZE];
  state.flags=FLAG(_distributed_unit_alive_bit)|FLAG(_distributed_unit_placed_bit);
  state.unit_index=7;state.vehicle_index=NONE;state.seat_index=NONE;
  state.scripted_view=TRUE;state.view_for_owner=scripted;
  for(unsigned i=0;i<7;i++)state.scripted_vectors[i]=(struct distributed_vector){32767,0,0};
  state.scripted_vectors[3]=(struct distributed_vector){123,-456,789};
  state.scripted_vectors[6]=(struct distributed_vector){-234,567,-890};
  word length=distributed_unit_state_write(&state,own,wire);
  assert(distributed_unit_state_read(wire,wire+length,&decoded)==length);
  real_vector3d before[7];memset(before,0,sizeof(before));
  for(unsigned i=0;i<7;i++)before[i]=(real_vector3d){0,1,0};
  fixture.object.parent_object_index=NONE;
  memcpy(&fixture.unit.desired_facing_vector,before,sizeof(before));
  coop=enabled;facing_calls=0;unit_queries=0;
  gamepkg_apply_scripted_view(7,&player,&decoded);
  boolean expected=enabled&&(!own||scripted);
  assert((unit_queries!=0)==expected);
  assert(facing_calls==(unsigned)(expected&&own));
  if(expected) {
   assert(fixture.unit.desired_facing_vector.i==1&&fixture.unit.desired_facing_vector.j==0);
   assert(fixture.unit.aiming_vector.i==1&&fixture.unit.looking_vector.i==1);
   assert(fabsf(fixture.unit.aiming_velocity.j-(-456/32767.0f))<1e-7f);
   assert(fabsf(fixture.unit.looking_velocity.k-(-890/32767.0f))<1e-7f);
   if(own)assert(last_facing.i==1&&last_facing.j==0);
  } else assert(!memcmp(&fixture.unit.desired_facing_vector,before,sizeof(before)));
  ++cases;
 }
 /* Invalid unit directions reject the whole snapshot before a unit query
    or any partial vector/camera write. Velocities are not unit directions. */
 for(unsigned invalid=0;invalid<7;invalid++) {
  if(invalid==3||invalid==6)continue;
  struct distributed_unit_state state={0};struct player_datum player={0,0};
  state.scripted_view=TRUE;
  for(unsigned i=0;i<7;i++)state.scripted_vectors[i]=(struct distributed_vector){32767,0,0};
  state.scripted_vectors[invalid]=(struct distributed_vector){0,0,0};
  real_vector3d before[7];memcpy(before,&fixture.unit.desired_facing_vector,sizeof(before));
  coop=TRUE;facing_calls=0;unit_queries=0;
  gamepkg_apply_scripted_view(7,&player,&state);
  assert(!unit_queries&&!facing_calls&&!memcmp(before,&fixture.unit.desired_facing_vector,sizeof(before)));
  ++cases;
 }
 printf("PASS %u actual codec-to-view applications: remote correction, active owner isolation, scripted owner camera, non-coop isolation, angular velocities, invalid-vector atomic rejection\n",cases);
}

int main(void) { transitions();snapshots();application();return 0; }
