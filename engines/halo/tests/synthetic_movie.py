"""Generate an original three-frame black/white Bink-k fixture, not game media."""
import struct
from pathlib import Path

def movie(audio=None,fps=(30,1)):
    # Revision k has an explicit constant-plane mode: one flag and an 8-bit
    # value, each plane padded to a 32-bit boundary. All frames are keyframes.
    packets=[struct.pack('<IIII',0,1|(y<<1),1|(128<<1),1|(128<<1)) for y in [0,255,0]]
    if audio:
        assert audio in ['rdft','dct']
        # An original all-zero 8000 Hz mono transform block. The two initial
        # coefficients, quantizers and zero-width coefficient runs are zero.
        # Both modes finish within the same 32-bit-aligned 72-byte packet.
        silence=struct.pack('<I',480*2)+bytes(68)
        packets=[struct.pack('<I',len(silence))+silence+packet for packet in packets]
    header=struct.pack('<4s11I',b'BIKk',0,3,len(packets[0]),3,16,16,*fps,0,int(bool(audio)),0)
    if audio:header+=struct.pack('<IHHI',1024,8000,0x1000 if audio=='dct' else 0,0)
    start=len(header)+4*len(packets)
    data=header+b''.join(struct.pack('<I',(start+i*len(packets[0]))|1) for i in range(3))+b''.join(packets)
    return data[:4]+struct.pack('<I',len(data)-8)+data[8:]

def entry(name,sector,size,directory=False,right=0):
    name=name.encode()
    return struct.pack('<HHIIBB',0,right,sector,size,16 if directory else 0,len(name))+name

def image(path,payload=None):
    payload=movie() if payload is None else payload
    root=entry('bink',43,64,True,right=8).ljust(32,b'\xff')+entry('maps',42,64,True)
    descriptor=bytearray(2048)
    descriptor[:20]=descriptor[0x7ec:0x800]=b'MICROSOFT*XBOX*MEDIA'
    struct.pack_into('<II',descriptor,0x14,40,64)
    with Path(path).open('wb') as f:
        for at,data in [(0x10000,descriptor),(40*2048,root.ljust(64,b'\xff')),
            (42*2048,entry('ui.map',44,10).ljust(64,b'\xff')),
            (43*2048,entry('intro.bik',45,len(payload)).ljust(64,b'\xff')),
            (44*2048,b'0123456789'),(45*2048,payload)]:
            f.seek(at);f.write(data)

if __name__=='__main__':
    import sys
    image(sys.argv[1])
