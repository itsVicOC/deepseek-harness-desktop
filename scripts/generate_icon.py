#!/usr/bin/env python3
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: generate_icon.py <output.png>")

    canvas = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)
    draw.rounded_rectangle((32, 32, 992, 992), radius=218, fill=(245, 247, 250, 255))
    draw.rounded_rectangle((184, 184, 840, 840), radius=150, fill=(14, 17, 21, 255))
    draw.rounded_rectangle((355, 726, 669, 754), radius=14, fill=(23, 105, 224, 255))

    font_path = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
    font = ImageFont.truetype(font_path, 260)
    bounds = draw.textbbox((0, 0), "DS", font=font)
    width = bounds[2] - bounds[0]
    height = bounds[3] - bounds[1]
    draw.text(((1024 - width) / 2, (1024 - height) / 2 - bounds[1] - 8), "DS", font=font, fill="white")

    output = Path(sys.argv[1])
    output.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(output, "PNG")


if __name__ == "__main__":
    main()
