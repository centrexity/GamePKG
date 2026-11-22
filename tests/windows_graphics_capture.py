"""Actual frozen Windows texture captures protect complete-viewport validation."""
from pathlib import Path
import hashlib,json
from PIL import Image
from windows_gui_support import texture_frame_error
from windows_native_gui import matches_frame
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

for size in [(960, 640), (640, 960)]:
 width,height=size
 frame=Image.new('RGB',size)
 frame.paste((0,0,255),(width//4,height//4,width*3//4,height*3//4))
 assert matches_frame(frame,(0,0,255))
 assert not matches_frame(frame,(0,255,0)),'stale image accepted on held-key frame'
 frame.putpixel((width//16,height//16),(255,0,0))
 assert not matches_frame(frame,(0,0,255)),'missing background clear accepted'
 assert not matches_frame(Image.new('RGB',size,(0,0,255)),(0,0,255)),'solid clear accepted as rendered image'
print('Native image oracle distinguishes image, input color, and clear on both client aspects.')
