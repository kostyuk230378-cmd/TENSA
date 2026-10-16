# -*- coding: utf-8 -*-
"""Генерация иконок PWA Tensa (ТЗ apps/pwa: icons 192/512).

Запуск: python scripts/make_icons.py

Рисуется тёмный квадрат с бирюзовой буквой «T» — двумя прямоугольниками,
без зависимости от системных шрифтов (нужен только Pillow).
"""

from pathlib import Path

from PIL import Image, ImageDraw

BG = (17, 24, 39)  # --bg из app.css (#111827)
FG = (56, 189, 248)  # --accent (#38bdf8)

OUT_DIR = Path(__file__).resolve().parent.parent / "static" / "icons"


def make_icon(size: int) -> Image.Image:
    """Иконка заданного размера: фон + буква «T»."""
    image = Image.new("RGB", (size, size), BG)
    draw = ImageDraw.Draw(image)

    bar_width = int(size * 0.56)
    bar_height = int(size * 0.14)
    stem_width = int(size * 0.16)
    stem_height = int(size * 0.44)

    top = int(size * 0.22)
    bar_left = (size - bar_width) // 2
    draw.rectangle(
        [bar_left, top, bar_left + bar_width, top + bar_height],
        fill=FG,
    )

    stem_left = (size - stem_width) // 2
    draw.rectangle(
        [stem_left, top + bar_height, stem_left + stem_width, top + bar_height + stem_height],
        fill=FG,
    )
    return image


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for size in (192, 512):
        path = OUT_DIR / f"icon-{size}.png"
        make_icon(size).save(path)
        print(f"{path} ({size}x{size})")


if __name__ == "__main__":
    main()
