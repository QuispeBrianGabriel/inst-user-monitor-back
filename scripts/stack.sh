#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Stack completo en local (front + API + Postgres) con las imágenes de
# producción, como en Railway. Ver docker/stack.yml.
#
# Se ejecuta en el HOST, no en el Dev Container: necesita Docker.
#
# Uso: scripts/stack.sh <up|down|logs|reset> [FRONT_DIR]
#   up     construye desde el código local, arranca, espera a la API y pasa
#          el smoke test (scripts/smoke.sh) contra la API y el origen del front
#   down   detiene el stack (los datos de la base se conservan)
#   logs   sigue los logs de los tres servicios
#   reset  detiene el stack y borra su base (pide confirmar)
#
# FRONT_DIR: repo del front. Por defecto busca un hermano de este repo
# (../front o ../inst-user-monitor).
# -----------------------------------------------------------------------------
source "$(dirname "$0")/_common.sh"

cd "$(dirname "$0")/.."

ACTION="${1:-up}"
FRONT_DIR="${2:-}"

export STACK_API_PORT="${STACK_API_PORT:-8088}"
export STACK_WEB_PORT="${STACK_WEB_PORT:-3001}"
API="http://localhost:$STACK_API_PORT"
WEB="http://localhost:$STACK_WEB_PORT"

if ! command -v docker >/dev/null || ! docker compose version >/dev/null 2>&1; then
	fail "Hace falta Docker con compose. Ejecuta make stack en el host, no en el Dev Container"
fi
if [[ ! -f docker/.env ]]; then
	fail "Falta docker/.env: ejecuta make setup"
fi

# La ruta del front se resuelve aquí, absoluta: compose interpreta las
# relativas respecto a docker/, no al directorio desde el que se llama.
if [[ -z "$FRONT_DIR" ]]; then
	for candidate in ../front ../inst-user-monitor; do
		if [[ -f "$candidate/docker/dockerfile.prod" ]]; then
			FRONT_DIR="$candidate"
			break
		fi
	done
fi
if [[ -z "$FRONT_DIR" || ! -f "$FRONT_DIR/docker/dockerfile.prod" ]]; then
	fail "No encuentro el repo del front${FRONT_DIR:+ en $FRONT_DIR}. Indícalo: make stack FRONT_DIR=/ruta/al/front"
fi
FRONT_DIR="$(cd "$FRONT_DIR" && pwd)"
export FRONT_DIR

compose() {
	docker compose -f docker/stack.yml "$@"
}

case "$ACTION" in
up)
	INTERRUPT_MSG="🛑 Arranque cancelado (make stack-down para detener lo que quedó en marcha)"
	info "Construyendo imágenes de producción (API: $PWD, front: $FRONT_DIR)"
	if ! compose up --build --detach; then
		fail "No se pudo arrancar el stack (revisa el error de arriba)"
	fi

	# La API migra la base al arrancar: puede tardar unos segundos.
	info "Esperando a la API en $API"
	for _ in $(seq 60); do
		if curl -fs -o /dev/null "$API/_readiness"; then
			break
		fi
		sleep 1
	done
	if ! curl -fs -o /dev/null "$API/_readiness"; then
		compose logs --tail 30 api
		fail "La API no responde en $API (logs de arriba; make stack-logs para seguirlos)"
	fi

	scripts/smoke.sh "$API" "$WEB"
	printf '\n  %sFront%s %s\n  %sAPI%s   %s\n\n' "$BOLD" "$RESET" "$WEB" "$BOLD" "$RESET" "$API"
	printf '%sLogs: make stack-logs · Detener: make stack-down%s\n' "$DIM" "$RESET"
	;;
down)
	compose down
	ok "Stack detenido (los datos se conservan; make stack-reset los borra)"
	;;
logs)
	INTERRUPT_MSG="⏹️  Fin de los logs (el stack sigue en marcha)"
	compose logs --follow --tail 50
	;;
reset)
	confirm "Se detendrá el stack y se borrará su base de datos. ¿Seguir?"
	compose down --volumes
	ok "Stack detenido y base borrada: el próximo make stack empieza de cero"
	;;
*)
	fail "Acción desconocida: $ACTION (up, down, logs o reset)"
	;;
esac
