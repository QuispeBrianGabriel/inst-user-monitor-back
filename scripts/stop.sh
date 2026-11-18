#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Detiene la API lanzada con `make run` o `make dev` y libera el 8080.
# Si no había ninguna corriendo, lo avisa sin fallar.
# -----------------------------------------------------------------------------
source "$(dirname "$0")/_common.sh"

BIN=target/debug/inst_user_monitor_back-cli

# cargo-watch relanza la API al detenerla: se para también el vigilante.
# pkill busca por la línea de comandos completa del proceso.
stopped=false
pkill -f cargo-watch && stopped=true
pkill -f "$BIN" && stopped=true

if $stopped; then
	ok "API detenida"
else
	printf '%sNo había ninguna API corriendo%s\n' "$DIM" "$RESET"
fi
