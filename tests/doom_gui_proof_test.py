"""Adversarial controls for the Windows DOOM integration verifier; no GUI."""
import struct, unittest
from PIL import Image
from doom_gui_proof import MAGIC, bundle, gamma
from windows_doom_gui import require_game_viewport

def original_bundle(config=b'usegamma 0\n'):
 slot=b'GUI SAVE\0'+bytes(2000)
 return MAGIC+struct.pack('<9I',0,len(slot),0,0,0,0,0,len(config),0)+slot+config

class ProofControls(unittest.TestCase):
 def test_checkpoint_and_original_gamma(self):
  data=original_bundle();self.assertTrue(bundle(data)[0].startswith(b'GUI SAVE\0'));self.assertEqual(gamma(data),0)
  for value in range(5):self.assertEqual(gamma(original_bundle(f'usegamma {value}\n'.encode())),value)
 def test_configuration_is_not_a_checkpoint(self):
  config=b'usegamma 0\n'
  data=MAGIC+struct.pack('<9I',0xffffffff,0,0,0,0,0,0,len(config),0)+config
  with self.assertRaises(AssertionError):bundle(data)
 def test_corrupt_or_oversized_slot_never_qualifies(self):
  original=original_bundle()
  probes=[b'',original[:43],b'badmagic'+original[8:],original[:-1],original+b'extra',bytes(4*1024*1024+1)]
  for offset,value in [(8,1),(12,0),(12,512*1024+1),(36,64*1024+1)]:
   damaged=bytearray(original);struct.pack_into('<I',damaged,offset,value);probes.append(bytes(damaged))
  for index,data in enumerate(probes):
   with self.subTest(index=index),self.assertRaises(AssertionError):bundle(data)
 def test_missing_duplicate_and_invalid_gamma_fail(self):
  for config in [b'',b'usegamma\n',b'usegamma 5\n',b'usegamma -1\n',b'usegamma 0\nusegamma 1\n',b'usegamma invalid\n']:
   with self.subTest(config=config),self.assertRaises((AssertionError,ValueError)):gamma(original_bundle(config))
 def test_game_fields_and_black_letterbox(self):
  # Independent fixed 800x600 RGB field, with 100-pixel side letterboxes in
  # a 1000x600 client. It is a verifier control, not an original-game capture.
  image=Image.new('RGB',(1000,600));field=Image.new('RGB',(800,600))
  field.putdata([((x%64)*4,(y%64)*4,128) for y in range(600) for x in range(800)])
  image.paste(field,(100,0));require_game_viewport(image)
  gray=image.copy();gray.paste((32,32,32),(0,0,100,600))
  with self.assertRaises(AssertionError):require_game_viewport(gray)
  for color in [(0,0,0),(128,128,128)]:
   with self.assertRaises(AssertionError):require_game_viewport(Image.new('RGB',(1000,600),color))
  thumbnail=Image.new('RGB',(1000,600));thumbnail.paste(field.resize((50,50)),(475,275))
  with self.assertRaises(AssertionError):require_game_viewport(thumbnail)

if __name__=='__main__':unittest.main()
