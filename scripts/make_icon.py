"""Genera assets/kero.ico, kero.png (256) y kero-1024.png: squircle grafito
con una lupa blanca fina.

Cada tamaño se dibuja por separado con supermuestreo 4x4 (no se reescala),
así los iconos pequeños quedan nítidos. Requiere Pillow:
    python scripts/make_icon.py
"""

import math
from pathlib import Path

from PIL import Image

SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]
SS = 4  # muestras por eje y píxel

# Degradado vertical grafito (arriba → abajo); lo bastante claro arriba
# para distinguirse sobre la barra de tareas oscura.
C0 = (64, 64, 68)
C1 = (24, 24, 26)
# Filo superior apenas más claro: separa el icono de fondos negros.
RIM = (96, 96, 100)
GLYPH = (245, 245, 247)

# Geometría en una rejilla de 32x32
HALF = 15.0  # semilado del squircle
EXP = 5.0  # exponente de la superelipse (forma "squircle")
LENS_C = (14.0, 14.0)
LENS_R = 6.0
STROKE = 1.15  # medio grosor del trazo
HANDLE = ((18.4, 18.4), (22.4, 22.4))


def in_squircle(x: float, y: float) -> bool:
    return (abs(x - 16) / HALF) ** EXP + (abs(y - 16) / HALF) ** EXP <= 1.0


def in_squircle_inset(x: float, y: float, d: float) -> bool:
    h = HALF - d
    return (abs(x - 16) / h) ** EXP + (abs(y - 16) / h) ** EXP <= 1.0


def in_glyph(x: float, y: float) -> bool:
    if abs(math.hypot(x - LENS_C[0], y - LENS_C[1]) - LENS_R) <= STROKE:
        return True
    (ax, ay), (bx, by) = HANDLE
    t = ((x - ax) * (bx - ax) + (y - ay) * (by - ay)) / ((bx - ax) ** 2 + (by - ay) ** 2)
    t = max(0.0, min(1.0, t))
    return math.hypot(x - (ax + t * (bx - ax)), y - (ay + t * (by - ay))) <= STROKE * 1.1


def draw(n: int, ss: int = SS) -> Image.Image:
    img = Image.new("RGBA", (n, n))
    px = img.load()
    s = n / 32.0
    total = ss * ss
    for py in range(n):
        for pxl in range(n):
            bg = fg = rim = 0
            for sy in range(ss):
                for sx in range(ss):
                    x = (pxl + (sx + 0.5) / ss) / s
                    y = (py + (sy + 0.5) / ss) / s
                    if in_squircle(x, y):
                        bg += 1
                        if in_glyph(x, y):
                            fg += 1
                        elif not in_squircle_inset(x, y, 0.6 if n >= 32 else 1.0):
                            rim += 1
            if bg == 0:
                continue
            k = (py + 0.5) / n
            base = [C0[i] * (1 - k) + C1[i] * k for i in range(3)]
            # El filo se desvanece hacia abajo: sólo luz cenital.
            r = rim / bg * max(0.0, 1.0 - 1.6 * k)
            base = [base[i] * (1 - r) + RIM[i] * r for i in range(3)]
            f = fg / bg
            rgb = tuple(round(base[i] * (1 - f) + GLYPH[i] * f) for i in range(3))
            px[pxl, py] = (*rgb, round(255 * bg / total))
    return img


def main() -> None:
    out = Path(__file__).resolve().parent.parent / "assets" / "kero.ico"
    out.parent.mkdir(exist_ok=True)
    images = [draw(n) for n in SIZES]
    images[-1].save(out, format="ICO", sizes=[(n, n) for n in SIZES], append_images=images[:-1])
    images[-1].save(out.with_suffix(".png"))
    # Fuente del .icns de macOS (Retina). Menos muestras: a este tamaño sobran.
    draw(1024, ss=2).save(out.with_name("kero-1024.png"))
    print(f"icono escrito en {out}")


if __name__ == "__main__":
    main()
