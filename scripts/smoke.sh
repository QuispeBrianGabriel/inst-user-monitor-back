#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Prueba rápida de que la API responde como se espera:
#   1. /health          → 200 (la API y la base responden)
#   2. /no-existe       → 404 (las rutas desconocidas no rompen nada)
#   3. /auth/me sin token → 401 (las rutas protegidas exigen token)
#
# Uso: scripts/smoke.sh [URL]
#   Por defecto apunta a la API local. Para producción: scripts/smoke.sh https://...
# -----------------------------------------------------------------------------
source "$(dirname "$0")/_common.sh"

URL="${1:-http://localhost:8080}"

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
expect "health" 200 "$URL/health"
expect "ruta desconocida" 404 "$URL/no-existe"
expect "auth sin token" 401 "$URL/auth/me"

if [[ $failures -gt 0 ]]; then
	fail "$failures comprobación(es) fallaron"
fi
ok "Todo responde como se espera"
