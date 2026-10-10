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
parser.add_argument('--features', action='store_true', help='Exercise audio DSP/mix, annotations, replay and auto-stop')
parser.add_argument('--segments', action='store_true', help='Exercise short independently finalized MKV parts')
parser.add_argument('--stop-paused', action='store_true', help='Stop while paused and verify paused time is excluded')
args = parser.parse_args()
if args.bundle is not None:
    args.bundle = args.bundle.resolve()

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
    if action == 'annotations':
        payload['items'] = [{'kind':'arrow', 'points':[[.1,.1],[.8,.8]], 'rgb':[255,80,80], 'width':3}]
    if action == 'save_replay':
        payload['path'] = str(replay_output)
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
replay_output = directory / f'replay-{uuid.uuid4().hex}.mkv'
if args.features:
    if args.format == 'gif': raise ValueError('Feature test requires H.264')
    config.update(mix_audio=True, noise_suppression=True, noise_gate=True, limiter=True,
                  max_seconds=6, replay_seconds=15, replay_mib=4, annotations_enabled=True)
if args.segments:
    config.update(segment_seconds=2, max_seconds=5)
if args.region:
    config['region'] = [0, 0, min(640, source['width']), min(360, source['height'])]
command('start', config)
deadline = time.monotonic() + 25
started = None
saved_replay = False
try:
    while time.monotonic() < deadline:
        status = read(library.recorder_status_json())
        if status['state'] == 'failed':
            raise RuntimeError(status.get('error'))
        if status['state'] == 'recording':
            if started is None:
                started = time.monotonic()
                if args.features: command('annotations')
                if args.pause:
                    command('pause')
                    time.sleep(.4)
                    assert read(library.recorder_status_json())['state'] == 'paused'
                    command('resume')
            if args.features and time.monotonic() - started > (1 if args.segments else 3) and not saved_replay:
                command('save_replay')
                saved_replay = True
            if args.stop_paused and time.monotonic() - started > 3:
                command('pause')
                time.sleep(1)
                command('stop')
                break
            if not args.features and not args.segments and time.monotonic() - started > 3:
                command('stop')
                break
        if (args.features or args.segments) and status['state'] == 'stopped': break
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
    if args.features:
        if not args.stop_paused: assert status.get('stop_reason'), status
        assert status.get('replay_output') == str(replay_output), status
        assert not status.get('replay_error'), status
        assert replay_output.exists() and status['replay_bytes'] <= 4 * 1024 * 1024
    if args.stop_paused:
        assert status['seconds'] < 3.6, status
    if args.segments:
        assert len(status['outputs']) >= 2, status
        assert all(Path(path).exists() for path in status['outputs']), status
    print(json.dumps({'file': str(output), 'frames': status['frames'],
                      'encoder': status['encoder'], 'state': status['state'],
                      'replay': str(replay_output) if args.features else None,
                      'outputs': status.get('outputs'),
                      'stop_reason': status.get('stop_reason')}))
finally:
    status = read(library.recorder_status_json())
    if status['state'] in ('preparing', 'recording', 'paused'):
        command('stop')
