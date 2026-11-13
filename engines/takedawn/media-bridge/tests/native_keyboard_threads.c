/* Window writers, listener replacement and simultaneous game pollers. */
#include "keyboard.h"
#include <SDL.h>
#include <assert.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { READERS=4, ROUNDS=50000 };
static SDL_atomic_t start;
static _Atomic unsigned callbacks;
static uint32_t packets[READERS*ROUNDS];
static void activity(void) {
    atomic_fetch_add_explicit(&callbacks,1,memory_order_relaxed);
    /* Hooks can inspect key state and replace themselves. No input lock
     * may be held while calling an application listener. */
    (void)xbox_FramebufferKeyDown('W');
    xbox_InputKeyboardSetActivityHook(activity);
}
static void await_start(void) { while(!SDL_AtomicGet(&start)) SDL_Delay(0); }
static int SDLCALL writer(void *unused) {
    (void)unused;await_start();
    for(unsigned i=0;i<ROUNDS;i++) {
        xbox_InputKeyboardSetActivityHook(i&1?activity:NULL);
        xbox_InputKeySet('W',1);xbox_InputKeySet(0x0d,1);
        if(i&1)xbox_InputKeysClear();
        else {xbox_InputKeySet('W',0);xbox_InputKeySet(0x0d,0);}
    }
    return 0;
}
static int SDLCALL listener_writer(void *unused) {
    (void)unused;await_start();
    for(unsigned i=0;i<ROUNDS;i++)xbox_InputKeyboardSetActivityHook(i&1?activity:NULL);
    return 0;
}
static int SDLCALL reader(void *slot) {
    unsigned index=(unsigned)(uintptr_t)slot;await_start();
    for(unsigned i=0;i<ROUNDS;i++) {
        XBOX_INPUT_STATE state;xbox_InputKeyboardState(&state);
        assert(!(state.Gamepad.wButtons&~XBOX_GAMEPAD_START));
        assert(state.Gamepad.bAnalogButtons[XBOX_BUTTON_RTRIGGER]==0||state.Gamepad.bAnalogButtons[XBOX_BUTTON_RTRIGGER]==255);
        packets[index*ROUNDS+i]=state.dwPacketNumber;
        (void)xbox_FramebufferKeyDown('W');
    }
    return 0;
}
static int compare(const void *a,const void *b) {
    uint32_t x=*(const uint32_t*)a,y=*(const uint32_t*)b;
    return (x>y)-(x<y);
}
int main(void) {
    assert(SDL_Init(0)==0);
    /* Initialize the immutable environment before starting workers. */
    XBOX_INPUT_STATE initial;assert(xbox_InputKeyboardPoll(&initial));
    SDL_Thread *threads[READERS+2];threads[0]=SDL_CreateThread(writer,"key-window",NULL);assert(threads[0]);
    threads[READERS+1]=SDL_CreateThread(listener_writer,"key-listener",NULL);assert(threads[READERS+1]);
    for(unsigned i=0;i<READERS;i++){threads[i+1]=SDL_CreateThread(reader,"key-poller",(void*)(uintptr_t)i);assert(threads[i+1]);}
    SDL_AtomicSet(&start,1);
    for(unsigned i=0;i<READERS+2;i++){int code=-1;SDL_WaitThread(threads[i],&code);assert(code==0);}
    xbox_InputKeyboardSetActivityHook(NULL);xbox_InputKeysClear();
    for(unsigned i=0;i<256;i++)assert(xbox_FramebufferKeyDown((int)i)==0);
    qsort(packets,READERS*ROUNDS,sizeof(uint32_t),compare);
    for(unsigned i=1;i<READERS*ROUNDS;i++)assert(packets[i]>packets[i-1]);
    assert(atomic_load_explicit(&callbacks,memory_order_relaxed)>0);
    SDL_Quit();puts("PASS concurrent keyboard: 200000 unique poll packets, key writes/clear, listener replacement and reentrant callbacks");return 0;
}
