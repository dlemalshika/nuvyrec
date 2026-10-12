"""Encode the supplied logo for launchers, keeping its artwork and alpha intact.

Requires Pillow. Uses transparent square padding rather than cropping/stretching.
Run with the original PNG path after updating assets/branding/nuvyrec-logo.png.
"""
from pathlib import Path
import argparse
from PIL import Image

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('source', type=Path)
args = parser.parse_args()
with Image.open(args.source) as original:
    logo = original.convert('RGBA')
side = max(logo.size)
square = Image.new('RGBA', (side, side), (0, 0, 0, 0))
square.alpha_composite(logo, ((side - logo.width) // 2, (side - logo.height) // 2))
square.save(root / 'apps/desktop/windows/runner/resources/app_icon.ico',
            sizes=[(n, n) for n in (16, 24, 32, 48, 64, 128, 256)])
# Inno's resource updater uses classic bitmap icon entries; the artwork is the
# same, but this container avoids compressed PNG entries in installer resources.
square.save(root / 'packaging/windows/installer_icon.ico', bitmap_format='bmp',
            sizes=[(n, n) for n in (16, 32, 48, 64, 128, 256)])
mac = root / 'apps/desktop/macos/Runner/Assets.xcassets/AppIcon.appiconset'
for size in (16, 32, 64, 128, 256, 512, 1024):
    square.resize((size, size), Image.Resampling.LANCZOS).save(mac / f'app_icon_{size}.png')
print('Windows ICO and macOS PNG launcher assets encoded; artwork preserved.')
