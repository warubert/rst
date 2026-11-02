#!/bin/bash
# Remove o RST instalado pelo pacote .pkg.
# Uso: sudo rst-uninstall

set -euo pipefail

if [ "$(id -u)" -ne 0 ]; then
  exec sudo "$0" "$@"
fi

link=/usr/local/bin/rst
app=/Applications/RST.app

if [ -L "$link" ]; then
  target="$(readlink "$link")"
  candidate="$(dirname "$(dirname "$(dirname "$target")")")"
  if [ "$(basename "$candidate")" = "RST.app" ]; then
    app="$candidate"
  fi
fi

printf 'Isto removerá o RST (%s). Continuar? [s/N] ' "$app"
read -r answer
case "$answer" in
  s | S | sim | y | Y) ;;
  *)
    echo "Cancelado."
    exit 0
    ;;
esac

rm -rf "$app"
rm -f /usr/local/bin/rst /usr/local/bin/rst-uninstall
pkgutil --forget com.warubert.rst >/dev/null 2>&1 || true

echo "RST removido. As configurações do usuário em ~/Library/Application Support/rst não foram apagadas."
