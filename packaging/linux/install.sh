#!/bin/sh
# Instala Kero para el usuario actual (sin root): binario en ~/.local/bin,
# lanzador en el menú de aplicaciones e icono. Volver a ejecutarlo actualiza.
set -eu

src=$(cd "$(dirname "$0")" && pwd)
bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
data="${XDG_DATA_HOME:-$HOME/.local/share}"

if [ -x "$bin/kero" ]; then
    "$bin/kero" --quit 2>/dev/null || true
fi

mkdir -p "$bin" "$data/applications" "$data/icons/hicolor/256x256/apps"
install -m 755 "$src/kero" "$bin/kero"
install -m 644 "$src/kero.png" "$data/icons/hicolor/256x256/apps/kero.png"
sed "s|^Exec=.*|Exec=$bin/kero|" "$src/kero.desktop" > "$data/applications/kero.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$data/applications" || true

echo "Kero instalado en $bin/kero"
case ":$PATH:" in
    *":$bin:"*) ;;
    *) echo "Aviso: $bin no está en tu PATH." ;;
esac
if [ "${XDG_SESSION_TYPE:-}" = "wayland" ]; then
    echo "Wayland no permite atajos globales a las apps: crea un atajo en la"
    echo "configuración de tu escritorio que ejecute:  $bin/kero --toggle"
fi
nohup "$bin/kero" >/dev/null 2>&1 &
