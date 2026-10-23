#include <initializer_list>
#include <cassert>
#include "../../engines/cube2/adapters/mouse.h"
struct vec {};
struct fpsent {
    bool attacking=false, ai=false;
    int lastaction=0, gunwait=0, lastattackgun=-1, gunselect=2;
};
static fpsent player;
static fpsent *player1=&player;
static bool connected=true, intermission=false;
static int attackspawn=0, lastmillis=1000, respawns=0;
static void respawn() { ++respawns; }
#include "original-click.h"
static void reset() { player=fpsent(); connected=true; intermission=false; attackspawn=0; lastmillis=1000; respawns=0; }
int main() {
    // Identical original attack gates receive raw edges or effective adapter edges.
    for(bool adapted: {false,true}) {
        reset();
        gamepkg_mouse_state mouse;
        auto edge=[&](bool down, bool controller=false) {
            if(!adapted || mouse.update(1,down,controller)) doattack(adapted ? mouse.down[1] : down);
        };
        edge(true); edge(false); shoot(player1,vec());
        assert(player.lastaction==0 && !player.attacking);
        edge(true); shoot(player1,vec());
        assert(player.lastaction==1000 && player.lastattackgun==2);
        edge(false); lastmillis=1100; shoot(player1,vec());
        assert(player.lastaction==1000);
        player.gunwait=200; edge(true); shoot(player1,vec());
        assert(player.lastaction==1000 && player.gunwait==200);
        lastmillis=1200; shoot(player1,vec());
        assert(player.lastaction==1200 && player.gunwait==0);
        edge(false);
        connected=false; edge(true); assert(!player.attacking);
        connected=true; intermission=true; edge(false); edge(true); assert(!player.attacking);
    }
    reset();
    gamepkg_mouse_state mouse;
    assert(mouse.update(1,true,false)); doattack(mouse.down[1]);
    assert(!mouse.update(1,true,false)); // No duplicate effective press.
    assert(!mouse.update(1,true,true));
    assert(!mouse.update(1,false,false));
    shoot(player1,vec()); assert(player.lastaction==1000);
    assert(mouse.update(1,false,true)); doattack(mouse.down[1]);
    lastmillis=1100; shoot(player1,vec()); assert(player.lastaction==1000);
}
