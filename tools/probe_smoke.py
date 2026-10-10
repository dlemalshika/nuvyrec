"""Probe and fully decode local test recordings; never display captured pixels."""
import argparse
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--bundle', type=Path, required=True)
parser.add_argument('--tracks', type=int, default=0)
parser.add_argument('files', type=Path, nargs='+')
args = parser.parse_args()
for path in args.files:
    result = subprocess.run([str(args.bundle.resolve() / 'ffprobe.exe'), '-v', 'error',
        '-show_streams', '-show_format', '-of', 'json', str(path.resolve())],
        capture_output=True, text=True, encoding='utf-8', check=True)
    metadata = json.loads(result.stdout)
    video = [s for s in metadata['streams'] if s['codec_type'] == 'video']
    audio = [s for s in metadata['streams'] if s['codec_type'] == 'audio']
    assert len(video) == 1 and video[0]['codec_name'] == 'h264', metadata
    assert len(audio) == args.tracks, metadata
    assert all(s['codec_name'] == 'aac' and s['channels'] == 2 and s['sample_rate'] == '48000' for s in audio)
    subprocess.run([str(args.bundle.resolve() / 'ffmpeg.exe'), '-v', 'error', '-i',
        str(path.resolve()), '-map', '0', '-f', 'null', '-'], check=True,
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    print(json.dumps({'file':path.name,'video':video[0]['codec_name'], 'audio_tracks':len(audio),
        'seconds':metadata['format'].get('duration'),'decode':'passed'}))
