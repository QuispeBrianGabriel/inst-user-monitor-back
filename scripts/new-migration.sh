#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Crea la siguiente migración de SQLx vacía, con el número correlativo.
#
# Uso: scripts/new-migration.sh <nombre>
#   Ej:  scripts/new-migration.sh crear_usuarios
#   →    migrations/0002_crear_usuarios.sql
#
# La API las aplica al arrancar (sqlx::migrate!). Una migración publicada
# nunca se edita ni se borra: SQLx guarda su checksum y fallaría al arrancar.
# -----------------------------------------------------------------------------
source "$(dirname "$0")/_common.sh"

NAME="${1:-}"

if [[ -z "$NAME" ]]; then
	fail "Falta el nombre. Ej: make migration NAME=crear_usuarios"
fi
if [[ ! "$NAME" =~ ^[a-z0-9_]+$ ]]; then
	fail "El nombre solo admite minúsculas, números y _"
fi

# --sequential: numera 0001, 0002… en lugar de usar la fecha.
if ! sqlx migrate add --sequential "$NAME"; then
	fail "sqlx no pudo crear la migración (revisa el error de arriba)"
fi
