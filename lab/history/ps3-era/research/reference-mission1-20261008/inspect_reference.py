"""Extract timestamped, private inspection stills from the user-supplied reference."""
import json
import subprocess
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).parent
SOURCE = Path(json.loads((ROOT / "source.json").read_text(encoding="utf-8-sig"))["source"])
FFMPEG = ROOT.parents[1] / "DEVLOGS/tools/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe"
TIMES = list(range(390, 536, 5)) + [610, 645, 710, 865, 1020, 1080, 1100, 1110, 1230, 1280, 1315, 1380, 1435, 1465, 1480, 1500]
font = ImageFont.truetype("C:/Windows/Fonts/arial.ttf", 24)
frames = []
for timestamp in TIMES:
    target = ROOT / f"at-{timestamp:04d}.png"
    subprocess.run([str(FFMPEG), "-hide_banner", "-loglevel", "error", "-ss", str(timestamp),
                    "-i", str(SOURCE), "-frames:v", "1", str(target)], check=True, timeout=30)
    frames.append((timestamp, target))
for offset in range(0, len(frames), 12):
    sheet = Image.new("RGB", (1920, 1600), "black")
    draw = ImageDraw.Draw(sheet)
    for j, (timestamp, path) in enumerate(frames[offset:offset + 12]):
        x, y = j % 3 * 640, j // 3 * 400
        sheet.paste(Image.open(path), (x, y))
        draw.text((x + 8, y + 365), f"{timestamp // 60:02d}:{timestamp % 60:02d}", font=font, fill="white")
    sheet.save(ROOT / f"details-{offset // 12 + 1}.jpg", quality=92)
print(json.dumps({"exact_seek_seconds": TIMES, "frames": len(frames), "media_private": True}))
