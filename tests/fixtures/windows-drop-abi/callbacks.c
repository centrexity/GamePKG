/* Maintainer ABI oracle: Windows SDK calls into the production Rust vtable.
 * No windows, game media or real file operations are involved. */
#define COBJMACROS
#include <oleidl.h>
#include <windows.h>
#include <shellapi.h>
#include <shlobj.h>
#include <string.h>

__declspec(dllexport) int __cdecl check_drop_abi(IDropTarget *target) {
  IDataObject *opaque = (IDataObject *)(UINT_PTR)0x1357;
  DWORD effect = 0;
  POINTL point = {1234568, -7654322};
  if (IDropTarget_DragEnter(target, opaque, 16, point, &effect) != S_OK ||
      effect != 1)
    return 1;
  point.x = 1234569;
  point.y = -7654323;
  if (IDropTarget_DragOver(target, 32, point, &effect) != S_OK || effect != 2)
    return 2;
  if (IDropTarget_DragLeave(target) != S_OK)
    return 3;
  point.x = 1234571;
  point.y = -7654325;
  if (IDropTarget_Drop(target, opaque, 64, point, &effect) != S_OK ||
      effect != 4)
    return 4;
  return 0;
}

/* A data provider retains ownership of its movable memory. The receiver must
 * release the medium through pUnkForRelease, never DragFinish/GlobalFree it. */
typedef struct Provider {
  IDataObject data;
  IUnknown owner;
  HGLOBAL memory;
  int acquired, released, freed_early, reject, direct;
} Provider;

static HRESULT STDMETHODCALLTYPE owner_query(IUnknown *self, REFIID iid, void **out) {
  (void)self; (void)iid;
  if (!out) return E_POINTER;
  *out = NULL;
  return E_NOINTERFACE;
}
static ULONG STDMETHODCALLTYPE owner_add(IUnknown *self) { (void)self; return 2; }
static ULONG STDMETHODCALLTYPE owner_release(IUnknown *self) {
  Provider *provider = CONTAINING_RECORD(self, Provider, owner);
  provider->released++;
  if (GlobalFlags(provider->memory) == GMEM_INVALID_HANDLE) provider->freed_early++;
  else GlobalFree(provider->memory);
  provider->memory = NULL;
  return 1;
}
static IUnknownVtbl owner_vtable = {owner_query, owner_add, owner_release};

static HRESULT STDMETHODCALLTYPE get_files(IDataObject *self, FORMATETC *format, STGMEDIUM *medium) {
  Provider *provider = CONTAINING_RECORD(self, Provider, data);
  WCHAR names[512];
  const WCHAR first[] = L"C:\\Games\\\x96ea.gamepkg";
  const WCHAR last[] = L"\\games.gamepkglist";
  SIZE_T length = sizeof(first) / sizeof(WCHAR), i;
  DROPFILES *files;
  if (format->cfFormat != CF_HDROP || format->tymed != TYMED_HGLOBAL ||
      format->dwAspect != DVASPECT_CONTENT || format->lindex != -1)
    return E_INVALIDARG;
  if (provider->reject) return DV_E_FORMATETC;
  if (provider->memory) return E_UNEXPECTED; /* Earlier medium leaked. */
  memcpy(names, first, sizeof(first));
  names[length++] = L'C'; names[length++] = L':'; names[length++] = L'\\';
  for (i = 0; i < 80; i++) {
    memcpy(names + length, L"long", 4 * sizeof(WCHAR)); length += 4;
  }
  memcpy(names + length, last, sizeof(last)); length += sizeof(last) / sizeof(WCHAR);
  names[length++] = 0;
  provider->memory = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, sizeof(DROPFILES) + length * sizeof(WCHAR));
  if (!provider->memory) return E_OUTOFMEMORY;
  files = (DROPFILES *)GlobalLock(provider->memory);
  if (!files) { GlobalFree(provider->memory); provider->memory = NULL; return E_OUTOFMEMORY; }
  files->pFiles = (DWORD)sizeof(DROPFILES); files->fWide = TRUE;
  memcpy((BYTE *)files + sizeof(DROPFILES), names, length * sizeof(WCHAR));
  GlobalUnlock(provider->memory);
  memset(medium, 0, sizeof(*medium));
  medium->tymed = TYMED_HGLOBAL;
  medium->hGlobal = provider->memory;
  medium->pUnkForRelease = provider->direct ? NULL : &provider->owner;
  provider->acquired++;
  return S_OK;
}

__declspec(dllexport) int __cdecl check_production_drop(IDropTarget *target, unsigned mode) {
  const IID unknown = {0, 0, 0, {0xc0, 0, 0, 0, 0, 0, 0, 0x46}};
  const IID drop = {0x122, 0, 0, {0xc0, 0, 0, 0, 0, 0, 0, 0x46}};
  const IID unsupported = {0xdeadbeef, 0, 0, {0}};
  void *out = (void *)(UINT_PTR)1;
  if (mode & 1) {
    if (IDropTarget_QueryInterface(target, &drop, NULL) != E_POINTER) return 10;
    if (IDropTarget_QueryInterface(target, NULL, &out) != E_POINTER || out) return 17;
    out = (void *)(UINT_PTR)1;
    if (IDropTarget_QueryInterface(target, &unsupported, &out) != E_NOINTERFACE || out) return 11;
    if (IDropTarget_QueryInterface(target, &unknown, &out) != S_OK || out != target) return 12;
    if (IUnknown_Release((IUnknown *)out) != 1) return 13;
    if (IDropTarget_QueryInterface(target, &drop, &out) != S_OK || out != target) return 14;
    if (IDropTarget_AddRef(target) != 3 || IDropTarget_Release(target) != 2) return 15;
    if (IDropTarget_Release((IDropTarget *)out) != 1) return 16;
  }
  if (mode & 2) {
    IDataObjectVtbl data_vtable = {0};
    Provider provider = {0};
    DWORD effect = DROPEFFECT_COPY;
    POINTL point = {123, -456};
    data_vtable.GetData = get_files;
    provider.data.lpVtbl = &data_vtable; provider.owner.lpVtbl = &owner_vtable;
    if (IDropTarget_DragEnter(target, &provider.data, 0, point, &effect) != S_OK || effect != DROPEFFECT_COPY) return 20;
    if (provider.acquired != 1 || provider.released != 1 || provider.memory || provider.freed_early) return 21;
    if (IDropTarget_DragLeave(target) != S_OK) return 22;
    if (IDropTarget_DragLeave(target) != S_OK) return 31;
    effect = DROPEFFECT_COPY;
    if (IDropTarget_Drop(target, &provider.data, 0, point, &effect) != S_OK || effect != DROPEFFECT_COPY) return 23;
    if (provider.acquired != 2 || provider.released != 2 || provider.memory || provider.freed_early) return 24;
    provider.direct = 1; effect = DROPEFFECT_COPY;
    if (IDropTarget_DragEnter(target, &provider.data, 0, point, &effect) != S_OK) return 25;
    if (GlobalFlags(provider.memory) != GMEM_INVALID_HANDLE) return 32;
    provider.memory = NULL;
    if (IDropTarget_DragLeave(target) != S_OK) return 26;
    provider.reject = 1; effect = DROPEFFECT_COPY;
    if (IDropTarget_DragEnter(target, &provider.data, 0, point, &effect) != S_OK || effect != DROPEFFECT_NONE) return 27;
    if (IDropTarget_DragLeave(target) != S_OK) return 28;
    effect = DROPEFFECT_COPY;
    if (IDropTarget_Drop(target, &provider.data, 0, point, &effect) != S_OK || effect != DROPEFFECT_NONE) return 29;
    if (provider.acquired != 3 || provider.released != 2 || provider.memory || provider.freed_early) return 30;
  }
  return 0;
}
