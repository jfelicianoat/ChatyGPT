from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[1]
ICONS = ROOT / "apps" / "desktop" / "src-tauri" / "icons"
CANVAS = 1024


def build_icon() -> Image.Image:
    image = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)

    for inset in range(0, 112):
        ratio = inset / 111
        color = (
            round(14 + 10 * ratio),
            round(23 + 18 * ratio),
            round(43 + 28 * ratio),
            255,
        )
        draw.rounded_rectangle(
            (inset, inset, CANVAS - inset, CANVAS - inset),
            radius=220 - inset,
            fill=color,
        )

    draw.arc(
        (248, 238, 776, 766),
        start=42,
        end=318,
        fill=(83, 231, 207, 255),
        width=116,
    )
    draw.arc(
        (286, 276, 738, 728),
        start=42,
        end=318,
        fill=(53, 126, 245, 255),
        width=38,
    )
    draw.ellipse((705, 268, 809, 372), fill=(245, 250, 255, 255))
    return image


def main() -> None:
    ICONS.mkdir(parents=True, exist_ok=True)
    image = build_icon()
    image.save(ICONS / "icon.png", format="PNG", optimize=True)
    image.save(
        ICONS / "icon.ico",
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )


if __name__ == "__main__":
    main()
