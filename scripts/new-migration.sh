#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Crea la siguiente migración con el generador de Loco: escribe el archivo
# migration/src/mAAAAMMDD_HHMMSS_<nombre>.rs y lo registra en migration/src/lib.rs.
#
# Uso: scripts/new-migration.sh <nombre>
#   Ej:  scripts/new-migration.sh add_name_to_users
#
# Después: edítala, aplícala (make migrate) y regenera las entidades
# (make entities). Una migración publicada nunca se edita ni se borra:
# crea otra que corrija la anterior.
# -----------------------------------------------------------------------------
source "$(dirname "$0")/_common.sh"

NAME="${1:-}"

if [[ -z "$NAME" ]]; then
	fail "Falta el nombre. Ej: make migration NAME=add_name_to_users"
fi
if [[ ! "$NAME" =~ ^[a-z0-9_]+$ ]]; then
	fail "El nombre solo admite minúsculas, números y _"
fi

if ! cargo loco generate migration "$NAME"; then
	fail "Loco no pudo crear la migración (revisa el error de arriba)"
fi
ok "Edítala, aplícala con make migrate y regenera las entidades con make entities"
