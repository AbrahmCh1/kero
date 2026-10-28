"""Genera assets/kero.ico: cuadrado redondeado azul con una lupa blanca.

Cada tamaño se dibuja por separado (no se reescala) para que los iconos
pequeños queden nítidos. Requiere Pillow:  python scripts/make_icon.py
"""

import math
from pathlib import Path

from PIL import Image

SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]
TOP = (36, 132, 240)
BOTTOM = (10, 92, 204)


def coverage(d: float, aa: float) -> float:
    return max(0.0, min(1.0, 0.5 - d / aa))


def draw(n: int) -> Image.Image:
    img = Image.new("RGBA", (n, n))
    px = img.load()
    s = n / 32.0  # diseño en una rejilla de 32x32
    aa = 1.0
    for y in range(n):
        for x in range(n):
            fx, fy = (x + 0.5) / s, (y + 0.5) / s
            # Cuadrado redondeado (SDF)
            hw, r = 15.0, 7.0
            qx = abs(fx - 16) - (hw - r)
            qy = abs(fy - 16) - (hw - r)
            outside = math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - r
            bg = coverage(outside * s, aa)
            if bg <= 0:
                continue
            # Lupa: anillo + mango
            ring = abs(math.hypot(fx - 14, fy - 14) - 6.5) - 1.7
            ax, ay, bx, by = 18.8, 18.8, 24.0, 24.0
            t = ((fx - ax) * (bx - ax) + (fy - ay) * (by - ay)) / ((bx - ax) ** 2 + (by - ay) ** 2)
            t = max(0.0, min(1.0, t))
            handle = math.hypot(fx - (ax + t * (bx - ax)), fy - (ay + t * (by - ay))) - 2.0
            fg = max(coverage(ring * s, aa), coverage(handle * s, aa))
            k = fy / 32.0
            base = [TOP[i] * (1 - k) + BOTTOM[i] * k for i in range(3)]
            rgb = tuple(round(base[i] * (1 - fg) + 255 * fg) for i in range(3))
            px[x, y] = (*rgb, round(bg * 255))
    return img


def main() -> None:
    out = Path(__file__).resolve().parent.parent / "assets" / "kero.ico"
    out.parent.mkdir(exist_ok=True)
    images = [draw(n) for n in SIZES]
    images[-1].save(out, format="ICO", sizes=[(n, n) for n in SIZES], append_images=images[:-1])
    images[-1].save(out.with_suffix(".png"))
    print(f"icono escrito en {out}")


if __name__ == "__main__":
    main()
