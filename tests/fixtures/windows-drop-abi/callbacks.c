/* Maintainer ABI oracle: Windows SDK calls into the production Rust vtable.
 * No windows, game media or real file operations are involved. */
#define COBJMACROS
#include <oleidl.h>
#include <windows.h>

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
