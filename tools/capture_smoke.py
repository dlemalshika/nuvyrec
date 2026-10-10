"""Exercise the C ABI and real native display capture without a Flutter UI.

Records a short video of the selected desktop. Outputs stay in artifacts/.
No recorded frames or window titles are printed or transmitted.
"""
import ctypes
import json
import os
from pathlib import Path
import time
import uuid
import argparse

parser = argparse.ArgumentParser()
parser.add_argument('--format', choices=['mkv','mp4','avi','gif'], default='mkv')
parser.add_argument('--region', action='store_true')
parser.add_argument('--pause', action='store_true')
parser.add_argument('--audio', choices=['none','system','microphone','both'], default='none')
parser.add_argument('--bundle', type=Path)
args = parser.parse_args()

root = Path(__file__).resolve().parent.parent
sdk = args.bundle or root / '.deps/ffmpeg/ffmpeg-8.1.2-full_build-shared/bin'
search = os.add_dll_directory(str(sdk))
library = ctypes.CDLL(str((args.bundle / 'recorder_core.dll') if args.bundle else (root / 'target/debug/recorder_core.dll')))
for symbol in ('recorder_sources_json', 'recorder_status_json'):
    getattr(library, symbol).restype = ctypes.c_void_p
library.recorder_command_json.argtypes = [ctypes.c_char_p]
library.recorder_command_json.restype = ctypes.c_void_p
library.recorder_string_free.argtypes = [ctypes.c_void_p]

def read(pointer):
    if not pointer:
        raise RuntimeError('Native allocation failed')
    try:
        return json.loads(ctypes.string_at(pointer).decode('utf-8'))
    finally:
        library.recorder_string_free(pointer)

def command(action, config=None):
    payload = {'action': action}
    if config is not None:
        payload['config'] = config
    response = read(library.recorder_command_json(json.dumps(payload).encode()))
    if not response.get('ok'):
        raise RuntimeError(response.get('error'))

sources = read(library.recorder_sources_json())['sources']
source = next(s for s in sources if s['kind'] == 'display')
directory = root / 'artifacts'
directory.mkdir(exist_ok=True)
output = directory / f'capture-{uuid.uuid4().hex}.{args.format}'
config = dict(output=str(output), container=args.format, source_id=source['id'],
              source_kind=source['kind'], width=640, height=360, fps=10 if args.format == 'gif' else 30,
              bitrate=2_000_000, encoder='auto')
config['system_audio'] = args.audio in ('system','both')
config['microphone'] = args.audio in ('microphone','both')
if args.region:
    config['region'] = [0, 0, min(640, source['width']), min(360, source['height'])]
command('start', config)
deadline = time.monotonic() + 25
started = None
try:
    while time.monotonic() < deadline:
        status = read(library.recorder_status_json())
        if status['state'] == 'failed':
            raise RuntimeError(status.get('error'))
        if status['state'] == 'recording':
            if started is None:
                started = time.monotonic()
                if args.pause:
                    command('pause')
                    time.sleep(.4)
                    assert read(library.recorder_status_json())['state'] == 'paused'
                    command('resume')
            if time.monotonic() - started > 3:
                command('stop')
                break
        time.sleep(.1)
    else:
        raise RuntimeError('Native capture did not start before timeout')
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        status = read(library.recorder_status_json())
        if status['state'] in ('stopped', 'failed'):
            break
        time.sleep(.1)
    if status['state'] != 'stopped':
        raise RuntimeError(f'Capture did not finalize: {status}')
    print(json.dumps({'file': str(output), 'frames': status['frames'],
                      'encoder': status['encoder'], 'state': status['state']}))
finally:
    status = read(library.recorder_status_json())
    if status['state'] in ('preparing', 'recording', 'paused'):
        command('stop')
