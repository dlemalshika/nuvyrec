"""Verify actual native Flutter Unicode/logo pixels reached an encoded video.
Only reads the explicitly generated, owned-window fixture; no desktop capture.
"""
import argparse
import json
import math
from pathlib import Path
import subprocess
parser=argparse.ArgumentParser()
parser.add_argument('--bundle',type=Path,required=True)
parser.add_argument('--require-gpu',action='store_true',help='Require D3D11 composition in the recorded fixture')
parser.add_argument('directory',type=Path)
args=parser.parse_args()
result=json.loads((args.directory/'result.json').read_text(encoding='utf-8'))
assert result['ok'],result
if args.require_gpu:assert result.get('compositor')=='d3d11',result
assert [result['width'],result['height'],result['fps']]==[640,360,24],result
width,height=result['width'],result['height']
video=Path(result['output'])
ffmpeg=args.bundle.resolve()/'ffmpeg.exe'
pixels=subprocess.check_output([str(ffmpeg),'-v','error','-i',str(video),'-frames:v','1','-f','rawvideo','-pix_fmt','rgb24','-'])
assert len(pixels)==width*height*3
def pixel(x,y):
    i=(y*width+x)*3
    return pixels[i:i+3]
def placement(meta):
    scale=min(width*meta['width']/meta['pixel_width'],height/meta['pixel_height'])
    w=max(1,min(width,math.floor(meta['pixel_width']*scale+.5)))
    h=max(1,min(height,math.floor(meta['pixel_height']*scale+.5)))
    left=math.floor((width-w)*meta['x']+.5)&~1
    top=math.floor((height-h)*meta['y']+.5)&~1
    return left,top,w,h
text=result['uploads'][0]
assert 'ආයුබෝවන්' in text['content']
data=Path(text['rgba_file']).read_bytes()
assert len(data)==text['pixel_width']*text['pixel_height']*4
left,top,w,h=placement(text)
matched=expected=0
for y in range(h):
    for x in range(w):
        source=((y*text['pixel_height']//h)*text['pixel_width']+x*text['pixel_width']//w)*4
        rgba=data[source:source+4]
        if min(rgba[:3])>225 and rgba[3]>240:
            expected+=1
            if min(pixel(left+x,top+y))>175:matched+=1
assert expected>100 and matched/expected>.85,{'glyph_pixels':expected,'matched':matched}
logo=result['uploads'][1]
left,top,w,h=placement(logo)
center=pixel(left+w//2,top+h//2)
assert center[0]>200 and center[1]>150 and center[2]<110,{'logo_center':list(center)}
# Transparent corner retains the blue synthetic background, not a black plate.
corner=pixel(left+2,top+2)
assert corner[2]>130 and corner[2]>corner[0]*2,{'transparent_logo_corner':list(corner)}
subprocess.run([str(ffmpeg),'-v','error','-i',str(video),'-map','0','-f','null','-'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
print(json.dumps({'native_flutter_unicode':'passed','logo_alpha':'passed','glyph_pixels':expected,'glyph_match_ratio':round(matched/expected,3),'frames':result['frames'],'decode':'passed','file':str(video)}))
