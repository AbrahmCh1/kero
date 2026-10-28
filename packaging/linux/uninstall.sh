#!/bin/sh
# Desinstala Kero. La configuración (~/.config/kero) se conserva.
set -eu

bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
config="${XDG_CONFIG_HOME:-$HOME/.config}"

if [ -x "$bin/kero" ]; then
    "$bin/kero" --quit 2>/dev/null || true
fi
rm -f "$bin/kero" \
      "$data/applications/kero.desktop" \
      "$data/icons/hicolor/256x256/apps/kero.png" \
      "$config/autostart/kero.desktop"
rm -rf "$data/kero"
echo "Kero desinstalado. Tu configuración sigue en $config/kero"
