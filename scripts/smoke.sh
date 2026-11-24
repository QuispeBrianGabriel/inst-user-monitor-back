#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Prueba rápida de que la API responde como se espera:
#   1. /_readiness      → 200 (la API y la base responden)
#   2. /no-existe       → 404 (las rutas desconocidas no rompen nada)
#   3. /auth/me sin token → 401 (las rutas protegidas exigen token)
#   4. login de un email inexistente → 401 (la tabla users existe: las
#      migraciones están aplicadas; sin ellas, 500). No crea nada.
#   5. preflight CORS desde ORIGIN → 200 y el origen permitido (CORS_ORIGIN)
#
# Uso: scripts/smoke.sh [URL] [ORIGIN]
#   Por defecto apunta a la API local y al front local. Para producción:
#   scripts/smoke.sh https://api... https://front...
# -----------------------------------------------------------------------------
source "$(dirname "$0")/_common.sh"

URL="${1:-http://localhost:8080}"
ORIGIN="${2:-http://localhost:3000}"

failures=0

# Compara el código HTTP obtenido con el esperado e imprime el resultado.
# Si la API no responde, curl devuelve "000" y la comprobación falla.
expect() {
	local name="$1" expected="$2"
	shift 2
	local got
	got=$(curl -s -o /dev/null -w '%{http_code}' "$@" || true)

	if [[ "$got" == "$expected" ]]; then
		printf '%s✔%s %-20s %s\n' "$GREEN" "$RESET" "$name" "$got"
	else
		printf '%s✘%s %-20s %s (esperado %s)\n' "$RED" "$RESET" "$name" "$got" "$expected"
		failures=$((failures + 1))
	fi
}

info "Probando $URL"
expect "readiness" 200 "$URL/_readiness"
expect "ruta desconocida" 404 "$URL/no-existe"
expect "auth sin token" 401 "$URL/auth/me"
expect "migraciones" 401 -X POST "$URL/auth/login" \
	-H 'content-type: application/json' \
	-d '{"email":"smoke-test@example.com","password":"not-a-real-password"}'
expect "preflight CORS" 200 -X OPTIONS "$URL/auth/register" \
	-H "origin: $ORIGIN" -H 'access-control-request-method: POST' \
	-H 'access-control-request-headers: content-type'

# El preflight responde aunque el origen no esté permitido: lo que cuenta es
# la cabecera. Sin ella el navegador bloquea la respuesta y el front muestra
# "No se pudo conectar con el servidor".
allowed=$(curl -s -o /dev/null -D - -X OPTIONS "$URL/auth/register" \
	-H "origin: $ORIGIN" -H 'access-control-request-method: POST' |
	tr -d '\r' | awk -F': ' 'tolower($1) == "access-control-allow-origin" { print $2 }' || true)
if [[ "$allowed" == "$ORIGIN" ]]; then
	printf '%s✔%s %-20s %s\n' "$GREEN" "$RESET" "CORS origen" "$ORIGIN"
else
	printf '%s✘%s %-20s %s (CORS_ORIGIN de la API debe incluir %s)\n' "$RED" "$RESET" "CORS origen" "${allowed:-ninguno}" "$ORIGIN"
	failures=$((failures + 1))
fi

if [[ $failures -gt 0 ]]; then
	fail "$failures comprobación(es) fallaron"
fi
ok "Todo responde como se espera"
