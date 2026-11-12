/* Native host-handle regression fixture; no game/ROM assets. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#define CHECK(x) do { if (!(x)) { fprintf(stderr,"FAIL line %d: %s (win32 %lu)\n",__LINE__,#x,(unsigned long)GetLastError()); exit(1); } } while (0)
#define KE_SHADOW_SIZE 4
static struct { uint32_t guest_va; HANDLE host_handle; uint32_t in_use; } g_ke_shadow[KE_SHADOW_SIZE];
static CRITICAL_SECTION g_ke_shadow_cs;
static uint32_t g_thread_obj_va=0x81000200, g_fs_base=0x1000;
static uint32_t fixture_tib_thread=0x81000400;
#define BRIDGE_MEM32(a) (*(CHECK_ADDRESS(a)))
static uint32_t *CHECK_ADDRESS(uint32_t a) { CHECK(a==g_fs_base+0x28);return &fixture_tib_thread; }
static LONG s_thobj_refs[2]={1,0};
static void thobj_lock(void) {}
static void thobj_unlock(void) {}
static int thobj_slot(uint32_t va) { return va==0x82000200?0:va==0x82000400?1:-1; }
static void thobj_unref(uint32_t va) { int i=thobj_slot(va);CHECK(i>=0 && s_thobj_refs[i]>1);s_thobj_refs[i]--; }
static void ke_shadow_init(void) {}
#include "thread_priority.inc.h"
static HANDLE done;
static DWORD WINAPI worker(void *unused) { (void)unused;WaitForSingleObject(done,INFINITE);return 0; }
int main(void)
{
    PROCESS_MITIGATION_STRICT_HANDLE_CHECK_POLICY strict={0};
    HANDLE thread,h,event;int owned;DWORD handles_before,handles_after;
    strict.RaiseExceptionOnInvalidHandleReference=1;strict.HandleExceptionsPermanentlyEnabled=1;
    CHECK(SetProcessMitigationPolicy(ProcessStrictHandleCheckPolicy,&strict,sizeof(strict)));
    InitializeCriticalSection(&g_ke_shadow_cs);
    done=CreateEventW(NULL,TRUE,FALSE,NULL);CHECK(done);
    thread=CreateThread(NULL,0,worker,NULL,0,NULL);CHECK(thread);
    CHECK(SetThreadPriority(thread,THREAD_PRIORITY_BELOW_NORMAL));
    g_ke_shadow[0].guest_va=0x82000200;g_ke_shadow[0].host_handle=thread;g_ke_shadow[0].in_use=1;
    event=CreateEventW(NULL,TRUE,FALSE,NULL);CHECK(event);
    g_ke_shadow[1].guest_va=0x83000000;g_ke_shadow[1].host_handle=event;g_ke_shadow[1].in_use=1;
    g_ke_shadow[2].guest_va=0x82000400;g_ke_shadow[2].host_handle=event;g_ke_shadow[2].in_use=1;
    h=bridge_priority_thread(g_thread_obj_va,&owned);CHECK(h==GetCurrentThread()&&!owned);
    CHECK(GetThreadPriority(h)!=THREAD_PRIORITY_ERROR_RETURN);
    h=bridge_priority_thread(fixture_tib_thread,&owned);CHECK(h==GetCurrentThread()&&!owned);
    CHECK(GetProcessHandleCount(GetCurrentProcess(),&handles_before));
    for(int i=0;i<1024;i++) {
        h=bridge_priority_thread(0x82000200,&owned);CHECK(h&&owned&&h!=thread);
        CHECK(GetThreadPriority(h)==THREAD_PRIORITY_BELOW_NORMAL);
        CHECK(s_thobj_refs[0]==1);CHECK(CloseHandle(h));
    }
    CHECK(GetProcessHandleCount(GetCurrentProcess(),&handles_after));CHECK(handles_before==handles_after);
    /* Events, retired objects, arbitrary pointers and NULL never reach Win32. */
    const uint32_t invalid[]={0,1,0xffffffff,0x83000000,0x82000400,0x80000000};
    for(unsigned i=0;i<sizeof(invalid)/sizeof(invalid[0]);i++) {owned=42;CHECK(!bridge_priority_thread(invalid[i],&owned)&&!owned);}
    h=bridge_priority_thread(0x82000200,&owned);CHECK(h&&owned);
    CHECK(CloseHandle(thread));g_ke_shadow[0].in_use=0;
    CHECK(GetThreadPriority(h)==THREAD_PRIORITY_BELOW_NORMAL);
    CHECK(SetThreadPriority(h,THREAD_PRIORITY_NORMAL));CHECK(GetThreadPriority(h)==THREAD_PRIORITY_NORMAL);
    CHECK(SetEvent(done));CHECK(WaitForSingleObject(h,5000)==WAIT_OBJECT_0);CHECK(CloseHandle(h));
    /* A live object whose host shadow was removed cannot become a raw handle. */
    CHECK(!bridge_priority_thread(0x82000200,&owned)&&!owned);CHECK(s_thobj_refs[0]==1);
    CHECK(CloseHandle(event));CHECK(CloseHandle(done));DeleteCriticalSection(&g_ke_shadow_cs);
    puts("PASS: current/TIB/other thread, priority, lifetime, 1024 duplicates without leaks, invalid/event/retired/missing objects; strict handles enabled");
    return 0;
}
