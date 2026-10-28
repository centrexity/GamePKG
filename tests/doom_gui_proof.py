"""Independent, bounded checks of the original engine's save-bundle format."""
import struct
MAGIC=b'\x89GPD2\r\n\x1a'
def bundle(data):
 assert 44<=len(data)<=4*1024*1024,'invalid DOOM bundle size'
 assert data[:8]==MAGIC,'not a versioned DOOM bundle'
 selected,*lengths=struct.unpack('<9I',data[8:44])
 assert selected==0,'checkpoint slot zero was not selected'
 assert sum(lengths)+44==len(data),'bundle length mismatch'
 assert all(n<=512*1024 for n in lengths[:6]) and all(n<=64*1024 for n in lengths[6:]),'bundle entry too large'
 assert lengths[0]>1000,'configuration-only bundle is not a checkpoint'
 parts=[];offset=44
 for length in lengths:parts.append(data[offset:offset+length]);offset+=length
 return parts

def gamma(data):
 parts=bundle(data)
 fields=[line.split(None,1) for line in parts[6].splitlines() if line.split(None,1)[:1]==[b'usegamma']]
 assert len(fields)==1 and len(fields[0])==2,'missing or duplicate original gamma'
 value=int(fields[0][1])
 assert 0<=value<=4,'invalid original gamma'
 return value
