"""Regenerate project-owned release icons: python -m pip install Pillow==12.3.0."""
from pathlib import Path
import struct

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent
SCALE = 4
image = Image.new("RGBA", (1024 * SCALE, 1024 * SCALE))
draw = ImageDraw.Draw(image)
draw.rounded_rectangle((32*SCALE, 32*SCALE, 992*SCALE, 992*SCALE),
                       radius=220*SCALE, fill="#172b3a")
lines = [((480, 512), (650, 300)), ((480, 512), (720, 512)),
         ((480, 512), (650, 724)), ((380, 372), (240, 512), (380, 652))]
for points in lines:
    draw.line([(x*SCALE, y*SCALE) for x, y in points],
              fill="#6de2c2", width=48*SCALE, joint="curve")
for x, y in ((650, 300), (720, 512), (650, 724)):
    draw.ellipse(((x-52)*SCALE, (y-52)*SCALE, (x+52)*SCALE, (y+52)*SCALE),
                 fill="#ecf8f5")
for size in (32, 128, 256, 512, 1024):
    image.resize((size, size), Image.Resampling.LANCZOS).save(ROOT / f"codeconvoy-{size}.png")
image.resize((256, 256), Image.Resampling.LANCZOS).save(
    ROOT / "codeconvoy.ico", sizes=[(s, s) for s in (16, 32, 48, 64, 128, 256)])
# Modern ICNS uses PNG chunks; no platform-specific generator is required.
chunks = b""
for kind, size in ((b"icp5", 32), (b"ic07", 128), (b"ic08", 256),
                   (b"ic09", 512), (b"ic10", 1024)):
    png = (ROOT / f"codeconvoy-{size}.png").read_bytes()
    chunks += kind + struct.pack(">I", len(png) + 8) + png
(ROOT / "codeconvoy.icns").write_bytes(b"icns" + struct.pack(">I", len(chunks) + 8) + chunks)
