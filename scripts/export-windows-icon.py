#!/usr/bin/env python3
"""Create the Windows icon from the original artwork (requires Pillow)."""

from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parent.parent
SIZES = (16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256)


def main():
    with Image.open(ROOT / "assets/app-icon.png") as source:
        artwork = source.convert("RGBA")

    # Preserve the composition and colors; only round the Windows tile's corners.
    # Supersample the mask so its edge stays smooth at every exported size.
    scale = 4
    width, height = artwork.size
    mask = Image.new("L", (width * scale, height * scale))
    ImageDraw.Draw(mask).rounded_rectangle(
        (0, 0, width * scale - 1, height * scale - 1),
        radius=round(min(width, height) * scale * 0.18),
        fill=255,
    )
    artwork.putalpha(mask.resize(artwork.size, Image.Resampling.LANCZOS))
    artwork.save(
        ROOT / "assets/app-icon.ico",
        sizes=[(size, size) for size in SIZES],
    )


if __name__ == "__main__":
    main()
