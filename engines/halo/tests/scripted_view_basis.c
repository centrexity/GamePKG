#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdint.h>
typedef float real;
typedef int boolean;
typedef union {struct{real i,j,k;};struct{real x,y,z;};} real_vector3d;
typedef real_vector3d real_point3d;
typedef struct {real yaw,pitch;} real_euler_angles2d;
typedef struct {real scale;real_vector3d forward,left,up;real_point3d position;} real_matrix4x3;
#define NONE (-1)
#define _pi 3.14159265358979323846f
#define _object_mask_vehicle 1
#define _unit_seat_third_person_camera_bit 0
#define TEST_FLAG(v,b) (((v)&(1u<<(b)))!=0)
#define TAG_BLOCK_GET_ELEMENT(block,index,type) (&(block)->elements[index])
#define cosine cosf
#define sine sinf
#define arctangent atan2f
#define square_root sqrtf
struct unit_seat {unsigned flags;};
struct unit_definition {struct{struct{struct unit_seat elements[1];}seats;}unit;};
struct players_vehicle_datum {long definition_index;struct{real_vector3d up;}object;};
struct unit_datum {struct{long parent_object_index;}object;struct{short parent_seat_index;}unit;};
struct player_datum {long unit_index;};
static struct player_datum player={3};
static struct unit_datum unit;
static struct players_vehicle_datum vehicle;
static struct unit_definition definition;
static int is_vehicle;
static real_vector3d right={.i=0,.j=-1,.k=0},down={.i=0,.j=0,.k=-1};
static const real_vector3d *global_right3d=&right,*global_down3d=&down;
static real_matrix4x3 parent_node={1,{.i=0,.j=1,.k=0},{.i=-1,.j=0,.k=0},{.i=0,.j=0,.k=1},{.i=0,.j=0,.k=0}};
static struct player_datum *player_get(long id){assert(id==7);return &player;}
static struct unit_datum *unit_get(long id){assert(id==3);return &unit;}
static void *object_try_and_get_and_verify_type(long id,int mask){assert(id==42&&mask==1);return is_vehicle?&vehicle:NULL;}
static struct unit_definition *vehicle_definition_get(long id){assert(id==0);return &definition;}
static real dot_product3d(const real_vector3d*a,const real_vector3d*b){return a->i*b->i+a->j*b->j+a->k*b->k;}
static real_vector3d *cross_product3d(const real_vector3d*a,const real_vector3d*b,real_vector3d*r){real_vector3d v={.i=a->j*b->k-a->k*b->j,.j=a->k*b->i-a->i*b->k,.k=a->i*b->j-a->j*b->i};*r=v;return r;}
static real normalize3d(real_vector3d*v){real n=sqrtf(dot_product3d(v,v));if(n){v->i/=n;v->j/=n;v->k/=n;}return n;}
static void set_real_point3d(real_point3d*p,real x,real y,real z){p->x=x;p->y=y;p->z=z;}
#include "candidate.inc"
int main(void){
 real_vector3d ups[]={{.i=0,.j=0,.k=1},{.i=.2f,.j=.4f,.k=.8944272f},{.i=0,.j=-1,.k=0},{.i=0,.j=1,.k=0},{.i=0,.j=0,.k=-1},{.i=.70710677f,.j=.70710677f,.k=0},{.i=.01f,.j=-.9999f,.k=.01f}};
 unsigned count=0;float worst=0;
 for(unsigned mode=0;mode<4;mode++)for(unsigned u=0;u<sizeof(ups)/sizeof(*ups);u++)for(int yaw=-180;yaw<=180;yaw+=15)for(int pitch=-80;pitch<=80;pitch+=10){
  unit.object.parent_object_index=mode==0?NONE:42;unit.unit.parent_seat_index=0;is_vehicle=mode>=2;definition.unit.seats.elements[0].flags=mode==3?1:0;vehicle.object.up=ups[u];normalize3d(&vehicle.object.up);
  real_euler_angles2d input={yaw*(_pi/180),pitch*(_pi/180)},recovered;real_vector3d world,local,again;
  player_aiming_vector_from_facing(7,&world,&input);gamepkg_scripted_view_local_facing(7,&world,&local);euler_angles2d_from_vector3d(&recovered,&local);player_aiming_vector_from_facing(7,&again,&recovered);
  real_vector3d delta={.i=again.i-world.i,.j=again.j-world.j,.k=again.k-world.k};float error=sqrtf(dot_product3d(&delta,&delta));if(error>worst)worst=error;assert(isfinite(error)&&error<0.00003f);count++;
 }
 printf("PASS %u original aiming/inverse roundtrips; no parent, nonvehicle, tilted and third-person seats; worst vector error %.9g\n",count,(double)worst);
 /* Keep the actual orthonormal inverse in the fixture for its negative control. */
 real_vector3d v={.i=1,.j=0,.k=0},out;matrix4x3_inverse_transform_normal(&parent_node,&v,&out);assert(out.j==-1);
 return 0;
}
