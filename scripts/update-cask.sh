#!/bin/sh
# Actualiza la versión y el SHA-256 del Cask de Kero.
# Uso: scripts/update-cask.sh <versión> [sha256]
set -eu

version=${1:?Uso: $0 <versión> [sha256]}
sha256=${2:-}
cask=${CASK_FILE:-Casks/kero-launcher.rb}

case "$version" in
    *[!0-9A-Za-z.+-]*|"")
        echo "Versión inválida: $version" >&2
        exit 1
        ;;
esac

if [ -z "$sha256" ]; then
    sums_url="https://github.com/AbrahmCh1/kero/releases/download/v$version/SHA256SUMS.txt"
    sums=$(curl -fsSL "$sums_url")
    sha256=$(printf '%s\n' "$sums" | awk -v asset="kero-$version-macos-universal.dmg" \
        '$2 == asset { print $1 }')
fi

case "$sha256" in
    *[!0-9a-f]*|"")
        echo "SHA-256 inválido: $sha256" >&2
        exit 1
        ;;
esac
if [ "${#sha256}" -ne 64 ]; then
    echo "SHA-256 inválido: $sha256" >&2
    exit 1
fi

VERSION="$version" SHA256="$sha256" perl -0pi -e '
    s/^  version "[^"]*"/  version "$ENV{VERSION}"/m or die "no se encontró version en el cask\n";
    s/^  sha256 "[0-9a-f]{64}"/  sha256 "$ENV{SHA256}"/m or die "no se encontró sha256 en el cask\n";
' "$cask"

grep -Fq "version \"$version\"" "$cask"
grep -Fq "sha256 \"$sha256\"" "$cask"
echo "Cask actualizado: $cask ($version)"
