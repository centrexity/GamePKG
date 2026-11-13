/* SPDX-License-Identifier: Apache-2.0
 * Asset-free harness around the actual upstream snapshot codec and setter. */
#include <assert.h>
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
struct unit_datum { struct { int type; real_vector3d translational_velocity; } object; struct { uint32_t flags; } unit; };
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
    for(unsigned flags=0;flags<256;flags++)for(unsigned own=0;own<2;own++)for(unsigned shape=0;shape<5;shape++)for(unsigned view=0;view<2;view++) {
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
        state.scripted_view=view;
        for(unsigned v=0;v<7;v++)state.scripted_vectors[v]=(struct distributed_vector){(short)(flags+v), (short)(32767-flags), (short)(-32768+flags)};
        word length=distributed_unit_state_write(&state,own,wire.data);
        assert(length>=DISTRIBUTED_UNIT_STATE_MINIMUM_SIZE&&length<=DISTRIBUTED_UNIT_STATE_MAXIMUM_SIZE);
        for(unsigned i=0;i<16;i++)assert(wire.before[i]==0xa5&&wire.after[i]==0xa5);
        for(unsigned i=length;i<sizeof(wire.data);i++)assert(wire.data[i]==0xa5);
        assert(distributed_unit_state_read(wire.data,wire.data+length,&decoded)==length);
        if(shape<3) {
            assert(decoded.unit_flags==flags);
            assert(decoded.scripted_view==view);
            if(view)assert(!memcmp(decoded.scripted_vectors,state.scripted_vectors,sizeof(state.scripted_vectors)));
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
int main(void) { transitions();snapshots();return 0; }
