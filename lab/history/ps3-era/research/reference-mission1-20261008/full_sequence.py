"""Private five-second sequence sampling; never substitutes for audio or every-frame review."""
import json, subprocess
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
ROOT = Path(__file__).parent
source = json.loads((ROOT / "source.json").read_text(encoding="utf-8-sig"))["source"]
ffmpeg = ROOT.parents[1] / "DEVLOGS/tools/imageio_ffmpeg/binaries/ffmpeg-win-x86_64-v7.1.exe"
folder = ROOT / "sequence"
folder.mkdir(exist_ok=False)
font = ImageFont.truetype("C:/Windows/Fonts/arial.ttf", 24)
times = list(range(0, 1521, 5))
for offset in range(0, len(times), 12):
    sheet = Image.new("RGB", (1920, 1600), "black")
    draw = ImageDraw.Draw(sheet)
    for i, second in enumerate(times[offset:offset + 12]):
        path = folder / f"at-{second:04d}.png"
        subprocess.run([str(ffmpeg), "-hide_banner", "-loglevel", "error", "-ss", str(second),
                        "-i", source, "-frames:v", "1", str(path)], check=True, timeout=30)
        x, y = i % 3 * 640, i // 3 * 400
        sheet.paste(Image.open(path), (x, y))
        draw.text((x + 8, y + 365), f"{second//60:02}:{second%60:02}", font=font, fill="white")
    sheet.save(folder / f"minute-{offset//12:02}.jpg", quality=92)
(folder / "sampling.json").write_text(json.dumps({"seconds": times, "interval_seconds": 5,
    "scope": "Full-duration five-second visual sampling, supplemented by closer cue stills; not every-frame or full audio review."}, indent=2) + "\n")
print(f"Extracted {len(times)} exact-seek stills across the whole reference.")
