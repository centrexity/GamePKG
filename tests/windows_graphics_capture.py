"""Actual frozen Windows texture captures protect complete-viewport validation."""
from pathlib import Path
import hashlib,json
from PIL import Image
from windows_gui_support import texture_frame_error
root=Path(__file__).resolve().parent/'reference/windows-native-graphics'
manifest=json.loads((root/'manifest.json').read_text())
for name,record in manifest['fixtures'].items():
 assert hashlib.sha256((root/name).read_bytes()).hexdigest()==record['sha256'],name
valid=Image.open(root/'resized-texture.png').convert('RGB')
assert texture_frame_error(valid,2) is None
assert texture_frame_error(valid,1) is not None,'Wrong input color passed'
assert texture_frame_error(valid,None) is not None,'Stale texture passed the transparent-black fallback check'
clipped=Image.open(root/'clipped-texture.png').convert('RGB')
error=texture_frame_error(clipped,2)
assert error is not None and 'Incomplete texture viewport' in error,error
print('Actual frozen Windows captures: resized texture accepted; wrong input color and clipped client image rejected; original captures unchanged.')
