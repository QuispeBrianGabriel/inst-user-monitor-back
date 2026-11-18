#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Ejecuta los tests. Los de integración arrancan la API contra la base de
# TEST_DATABASE_URL (ver config/test.yaml): si no existe, la crea antes.
#
# Uso: scripts/test.sh [filtro]
#   Ej:  scripts/test.sh login   → solo los tests cuyo nombre contiene "login"
# -----------------------------------------------------------------------------
source "$(dirname "$0")/_common.sh"

INTERRUPT_MSG="🛑 Tests interrumpidos"

ensure_test_db

if ! cargo test --locked "$@"; then
	fail "Hay tests que fallan: revisa la salida de arriba"
fi
ok "Todos los tests pasan"
