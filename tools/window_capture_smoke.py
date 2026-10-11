"""Verify full-window WGC preview/recording and live GPU cropping.

Creates only an owned, synthetic, non-activating Win32 window. Never captures
another application's contents, moves the pointer, or changes another window.
Pass --library and --pan-probe built against the bundled FFmpeg 8.1 SDK.
"""
import argparse
import ctypes as c
from ctypes import wintypes as w
import json
import os
from pathlib import Path
import subprocess
import threading
import time
import uuid
import socket

parser = argparse.ArgumentParser()
parser.add_argument('--library', type=Path, required=True)
parser.add_argument('--pan-probe', type=Path, required=True)
parser.add_argument('--gpu-probe', type=Path, help='Validate GPU overlays with synthetic camera pixels; no real camera is opened')
parser.add_argument('--webcam-preview', action='store_true', help='Opt-in real camera: verify shared preview/recording and live overlay hide/show; no thumbnail is saved or displayed')
parser.add_argument('--multistream', action='store_true', help='Use three loopback receivers to verify native FFI live control and silent AAC; no account or audio device is accessed')
args = parser.parse_args()
if args.multistream and args.webcam_preview:
    parser.error('Run synthetic multistream and real camera checks separately')
root = Path(__file__).resolve().parent.parent
sdk = root / '.deps/ffmpeg/ffmpeg-8.1.2-full_build-shared/bin'
search = os.add_dll_directory(str(sdk))
os.environ['PATH'] = str(sdk) + os.pathsep + os.environ['PATH']
user = c.WinDLL('user32', use_last_error=True)
gdi = c.WinDLL('gdi32', use_last_error=True)
kernel = c.WinDLL('kernel32', use_last_error=True)
PROC = c.WINFUNCTYPE(c.c_ssize_t, w.HWND, w.UINT, w.WPARAM, w.LPARAM)
class WindowClass(c.Structure):
    _fields_ = [('style',w.UINT),('proc',PROC),('class_extra',c.c_int),('window_extra',c.c_int),
                ('instance',w.HINSTANCE),('icon',w.HICON),('cursor',w.HANDLE),('background',w.HBRUSH),
                ('menu',w.LPCWSTR),('name',w.LPCWSTR)]
user.DefWindowProcW.argtypes = [w.HWND,w.UINT,w.WPARAM,w.LPARAM]
user.DefWindowProcW.restype = c.c_ssize_t
user.CreateWindowExW.argtypes = [w.DWORD,w.LPCWSTR,w.LPCWSTR,w.DWORD,c.c_int,c.c_int,c.c_int,c.c_int,w.HWND,w.HMENU,w.HINSTANCE,c.c_void_p]
user.CreateWindowExW.restype = w.HWND
user.GetDC.argtypes=[w.HWND]; user.GetDC.restype=w.HDC
user.ReleaseDC.argtypes=[w.HWND,w.HDC]
user.FillRect.argtypes=[w.HDC,c.POINTER(w.RECT),w.HBRUSH]
user.InvalidateRect.argtypes=[w.HWND,c.c_void_p,w.BOOL]
user.ValidateRect.argtypes=[w.HWND,c.c_void_p]
user.PostMessageW.argtypes=[w.HWND,w.UINT,w.WPARAM,w.LPARAM]
user.ShowWindow.argtypes=[w.HWND,c.c_int]
user.DestroyWindow.argtypes=[w.HWND]
user.SetTimer.argtypes=[w.HWND,c.c_size_t,w.UINT,c.c_void_p]
user.SetThreadDpiAwarenessContext.argtypes=[c.c_void_p]
kernel.GetModuleHandleW.argtypes=[w.LPCWSTR];kernel.GetModuleHandleW.restype=w.HMODULE
gdi.CreateSolidBrush.argtypes=[w.DWORD];gdi.CreateSolidBrush.restype=w.HBRUSH
gdi.DeleteObject.argtypes=[w.HGDIOBJ]
ready=threading.Event()
fixture={}
stream_receivers=[]
@PROC
def procedure(hwnd,message,wp,lp):
    if message==0x000f:  # WM_PAINT; colored quadrants reach every client edge
        dc=user.GetDC(hwnd)
        for rectangle,color in [((0,0,320,360),0x000000ff),((320,0,640,360),0x00ff0000),
                                ((0,280,80,360),0x0000ff00),((560,280,640,360),0x0000ffff)]:
            brush=gdi.CreateSolidBrush(color)
            user.FillRect(dc,c.byref(w.RECT(*rectangle)),brush)
            gdi.DeleteObject(brush)
        user.ReleaseDC(hwnd,dc)
        user.ValidateRect(hwnd,None)
        return 0
    if message==0x0113:
        user.InvalidateRect(hwnd,None,False)
        return 0
    if message==0x0010:
        user.DestroyWindow(hwnd)
        return 0
    if message==0x0002:
        user.PostQuitMessage(0)
        return 0
    return user.DefWindowProcW(hwnd,message,wp,lp)
def window_thread():
    user.SetThreadDpiAwarenessContext(c.c_void_p(-4))
    instance=kernel.GetModuleHandleW(None)
    name='FrameOwnedCaptureFixture'+uuid.uuid4().hex
    cls=WindowClass(0,procedure,0,0,instance,None,None,None,None,name)
    if not user.RegisterClassW(c.byref(cls)): raise c.WinError(c.get_last_error())
    style=0x00cf0000
    rect=w.RECT(0,0,640,360)
    user.AdjustWindowRect(c.byref(rect),style,False)
    hwnd=user.CreateWindowExW(0x08000000,name,'Frame synthetic capture fixture',style,
                              80,80,rect.right-rect.left,rect.bottom-rect.top,None,None,instance,None)
    if not hwnd: raise c.WinError(c.get_last_error())
    fixture['hwnd']=hwnd
    user.ShowWindow(hwnd,4) # SW_SHOWNOACTIVATE
    user.SetTimer(hwnd,1,30,None)
    ready.set()
    msg=w.MSG()
    while user.GetMessageW(c.byref(msg),None,0,0)>0:
        user.TranslateMessage(c.byref(msg)); user.DispatchMessageW(c.byref(msg))
thread=threading.Thread(target=window_thread,daemon=True)
thread.start()
assert ready.wait(5),'Fixture window failed to initialize'
library=c.CDLL(str(args.library.resolve()))
for name in ['recorder_sources_json','recorder_status_json']:
    getattr(library,name).restype=c.c_void_p
for name in ['recorder_command_json','recorder_preview_request_json']:
    getattr(library,name).argtypes=[c.c_char_p];getattr(library,name).restype=c.c_void_p
library.recorder_string_free.argtypes=[c.c_void_p]
library.recorder_preview_png.argtypes=[c.POINTER(c.c_size_t)];library.recorder_preview_png.restype=c.c_void_p
library.recorder_preview_free.argtypes=[c.c_void_p,c.c_size_t]
def read(pointer):
    assert pointer
    try: return json.loads(c.string_at(pointer).decode())
    finally: library.recorder_string_free(pointer)
def command(action,config=None,**extra):
    return read(library.recorder_command_json(json.dumps({'action':action,**({'config':config} if config else {}),**extra}).encode()))

preview_frames=0
camera_request=None
def camera_frame():
    global preview_frames
    size=c.c_size_t();width=c.c_uint32();height=c.c_uint32()
    pointer=library.recorder_webcam_preview_rgba(c.byref(size),c.byref(width),c.byref(height))
    if not pointer:return False
    try:
        assert 0<width.value<=320 and 0<height.value<=240
        assert size.value==width.value*height.value*4
        preview_frames+=1
        # Validate the binary ownership contract without storing or showing pixels.
        assert len(c.string_at(pointer,size.value))==size.value
    finally:library.recorder_preview_free(pointer,size.value)
    return True
directory=root/'artifacts'/('window-fixture-'+uuid.uuid4().hex)
directory.mkdir()
def rgb(path):
    data=subprocess.check_output([str(sdk/'ffmpeg.exe'),'-v','error','-i',str(path),'-vf','scale=160:180','-frames:v','1','-f','rawvideo','-pix_fmt','rgb24','-'])
    assert len(data)==160*180*3
    return lambda x,y: tuple(data[(y*160+x)*3:(y*160+x)*3+3])
def is_red(p):return p[0]>180 and p[1]<80 and p[2]<80
def is_blue(p):return p[2]>180 and p[0]<80 and p[1]<80
try:
    source=next(s for s in read(library.recorder_sources_json())['sources'] if s['kind']=='window' and s['id']==fixture['hwnd'])
    assert not source['minimized'] and source['width']>=640 and source['height']>360
    config=dict(output=str(directory/'full-window.mkv'),container='mkv',source_kind='window',source_id=source['id'],
                width=source['width']//2*2,height=source['height']//2*2,fps=30,bitrate=2_000_000,encoder='libx264')
    if args.multistream:
        targets=[]
        for index,platform in enumerate(['youtube','facebook','tiktok']):
            with socket.socket() as sock:
                sock.bind(('127.0.0.1',0));port=sock.getsockname()[1]
            server=f'rtmp://127.0.0.1:{port}/live'
            targets.append(dict(platform=platform,server=server,key='frame-owned-test-key'))
            child=subprocess.Popen([str(sdk/'ffmpeg.exe'),'-nostdin','-v','error',
                '-analyzeduration','500000','-probesize','32768','-listen','1','-i',server+'/frame-owned-test-key',
                '-c','copy','-flush_packets','1','-f','flv',str(directory/f'stream-{index}.flv')],
                stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            stream_receivers.append(child)
        config.update(streaming=targets)
        time.sleep(.6)
    if args.webcam_preview:
        library.recorder_cameras_json.restype=c.c_void_p
        library.recorder_webcam_preview_request_json.argtypes=[c.c_char_p];library.recorder_webcam_preview_request_json.restype=c.c_void_p
        library.recorder_webcam_preview_rgba.argtypes=[c.POINTER(c.c_size_t),c.POINTER(c.c_uint32),c.POINTER(c.c_uint32)];library.recorder_webcam_preview_rgba.restype=c.c_void_p
        cameras=read(library.recorder_cameras_json());assert cameras['ok'] and cameras['devices'],'No camera available for opt-in integration test'
        webcam=dict(enabled=True,device=cameras['devices'][0]['id'],x=.5,y=.1,width=.12,mirror=True,key=False,similarity=.15,softness=.08)
        camera_request=dict(webcam=webcam,low_resource=True)
        response=read(library.recorder_webcam_preview_request_json(json.dumps(camera_request).encode()));assert response['ok'],response
        deadline=time.monotonic()+12
        while time.monotonic()<deadline:
            response=read(library.recorder_webcam_preview_request_json(json.dumps(camera_request).encode()));assert response['ok'] and not response.get('error'),response
            if camera_frame():break
            time.sleep(.2)
        else:raise RuntimeError('Real webcam preview did not arrive')
        config.update(webcam=webcam,performance_mode='eco')
    response=command('start',config);assert response['ok'],response
    hidden=shown=False
    streams_stopped=False
    deadline=time.monotonic()+12
    while time.monotonic()<deadline:
        status=read(library.recorder_status_json())
        if status['state']=='failed':raise RuntimeError(status['error'])
        if args.multistream and status['state']=='recording':
            assert 'frame-owned-test-key' not in json.dumps(status),'Credentials leaked into status'
            if status['seconds']>3 and not streams_stopped:
                streams=status.get('streaming',[])
                assert len(streams)==3 and all(s['state']=='sending' and s['packets']>0 for s in streams),streams
                assert not command('pause')['ok'],'Pause interrupted live timestamps'
                assert command('stop_streaming')['ok']
                streams_stopped=True
            if status['seconds']>5:
                assert streams_stopped and not status['streaming_active'],status
                break
        elif args.webcam_preview and status['state']=='recording':
            assert not status.get('webcam_error'),status.get('webcam_error')
            camera_frame()
            if status['seconds']>.4 and not hidden:
                assert command('webcam_visible',enabled=False)['ok'];hidden=True
            if status['seconds']>1 and hidden and not shown:
                assert command('webcam_visible',enabled=True)['ok'];shown=True
            if status['seconds']>2.2:
                assert hidden and shown and preview_frames>=5 and status['webcam_ready'] and status['webcam_visible'],status
                break
        elif status['state']=='recording' and status['seconds']>1:break
        time.sleep(.1)
    else:raise RuntimeError('Owned window did not record')
    assert command('stop')['ok']
    deadline=time.monotonic()+10
    while time.monotonic()<deadline and read(library.recorder_status_json())['state']!='stopped':time.sleep(.1)
    assert read(library.recorder_status_json())['state']=='stopped'
    if args.multistream:
        for index,child in enumerate(stream_receivers):
            child.wait(timeout=8)
            path=directory/f'stream-{index}.flv'
            info=json.loads(subprocess.check_output([str(sdk/'ffprobe.exe'),'-v','error','-show_streams','-show_format','-of','json',str(path)]))
            assert [s['codec_name'] for s in info['streams']]==['h264','aac'],info
            assert 2<float(info['format']['duration'])<4.5,info
            subprocess.run([str(sdk/'ffmpeg.exe'),'-nostdin','-v','error','-i',str(path),'-f','null','-'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
        print('Native FFI multistream: three receivers, silent AAC, secret-free status, pause guard and Stop live passed')
    if args.gpu_probe:
        subprocess.run([str(args.gpu_probe.resolve()),str(fixture['hwnd']),str(directory)],check=True)
        def gpu_rgb(label):
            data=subprocess.check_output([str(sdk/'ffmpeg.exe'),'-v','error','-i',str(directory/f'gpu-{label}.png'),'-frames:v','1','-f','rawvideo','-pix_fmt','rgb24','-'])
            assert len(data)==320*180*3
            return lambda x,y:tuple(data[(y*320+x)*3:(y*320+x)*3+3])
        composed=gpu_rgb('composite')
        assert composed(160,45)[1]>220 and composed(160,45)[0]<30,'GPU line not drawn'
        assert is_red(composed(10,120)),'Transparent GPU raster damaged background'
        assert all(abs(a-b)<12 for a,b in zip(composed(60,120),(255,128,128))),'GPU raster alpha incorrect'
        assert composed(260,120)[0]>220 and composed(260,120)[2]>220,'GPU webcam mirror incorrect'
        assert is_blue(composed(310,120)),'GPU chroma key did not preserve background'
        cleared=gpu_rgb('clear')
        assert is_blue(cleared(260,120)) and is_blue(cleared(160,45)),'GPU clear retained stale overlays'
        spot=gpu_rgb('spotlight')
        assert is_red(spot(80,45)) and 65<spot(200,45)[2]<100,'GPU spotlight shading incorrect'
        print('GPU composition: geometry, raster alpha, synthetic camera mirror/key, clear and spotlight passed')
    # Source-size aspect thumbnail, returned independently of the media thread.
    ratio=min(480/source['width'],270/source['height'])
    request=dict(id=source['id'],width=round(source['width']*ratio),height=round(source['height']*ratio))
    deadline=time.monotonic()+8
    while time.monotonic()<deadline:
        response=read(library.recorder_preview_request_json(json.dumps(request).encode()))
        assert response['ok'],response
        length=c.c_size_t()
        pointer=library.recorder_preview_png(c.byref(length))
        if pointer:
            try:(directory/'preview.png').write_bytes(c.string_at(pointer,length.value))
            finally:library.recorder_preview_free(pointer,length.value)
            break
        time.sleep(.2)
    else:raise RuntimeError(f'Window thumbnail did not arrive: {response}')
    for path in [directory/'preview.png',directory/'full-window.mkv']:
        pixel=rgb(path)
        assert is_red(pixel(40,80)) and is_blue(pixel(120,80)),str(path)
        assert pixel(8,165)[1]>180 and pixel(152,165)[0]>180 and pixel(152,165)[1]>180,'Bottom corners were cropped'
        assert not is_red(pixel(40,3)),'Title bar missing'
    user.ShowWindow(fixture['hwnd'],6)
    time.sleep(.3)
    minimized=next(s for s in read(library.recorder_sources_json())['sources'] if s['id']==source['id'] and s['kind']=='window')
    assert minimized['minimized']
    config['output']=str(directory/'must-not-exist.mkv')
    assert not command('start',config)['ok']
    assert not Path(config['output']).exists()
    user.ShowWindow(fixture['hwnd'],4)
    time.sleep(.3)
    subprocess.run([str(args.pan_probe.resolve()),str(fixture['hwnd']),str(directory)],check=True)
    assert is_red(rgb(directory/'pan-left.png')(80,80)),'Left GPU crop incorrect'
    assert is_blue(rgb(directory/'pan-right.png')(80,80)),'Live GPU panning did not change the crop'
    if args.webcam_preview:
        subprocess.run([str(sdk/'ffmpeg.exe'),'-v','error','-i',str(directory/'full-window.mkv'),'-f','null','-'],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
    print(json.dumps({'result':'passed','full_window':True,'native_preview':True,'minimized_rejected':True,'same_graph_pan':True,'webcam_preview_frames':preview_frames,'webcam_hide_show':hidden and shown,'fixture_artifacts':str(directory)}))
finally:
    command('stop')
    for child in stream_receivers:
        if child.poll() is None:
            child.terminate();child.wait(timeout=5)
    if camera_request is not None:
        read(library.recorder_webcam_preview_request_json(json.dumps(dict(webcam=dict(enabled=False),low_resource=True)).encode()))
    user.PostMessageW(fixture['hwnd'],0x0010,0,0)
    thread.join(3)
