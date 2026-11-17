#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Ejecuta los mismos pasos que el Pipeline (.github/workflows/pipeline.yml),
# en el mismo orden, y se detiene en el primero que falle.
#
#   1. format → cargo fmt --check
#   2. lint   → cargo clippy (warnings como errores, ver .cargo/config.toml)
#   3. test   → cargo test
#   4. build  → cargo build --release
# -----------------------------------------------------------------------------
source "$(dirname "$0")/_common.sh"

INTERRUPT_MSG="🛑 Comprobación interrumpida"

step() {
	local name="$1"
	shift
	info "$name"
	if ! cargo "$@"; then
		fail "Falló el paso '$name': revisa la salida de arriba"
	fi
}

step format fmt --check
step lint clippy --locked --all-targets
step test test --locked
step build build --locked --release

ok "Todo en orden: el Pipeline debería pasar"
