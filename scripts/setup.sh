#!/usr/bin/env bash
# -----------------------------------------------------------------------------
# Prepara un repositorio recién clonado para abrirlo en el Dev Container.
#
# ⚠ Es el ÚNICO script que se ejecuta en el HOST, fuera del Dev Container,
#   porque deja lista la configuración que el contenedor necesita para arrancar.
#
# Pasos:
#   1. Comprueba que estás en el host y que Docker responde.
#   2. Crea docker/.env a partir de docker/.env.example (si no existe).
#   3. Cambia cada contraseña `change-me` por una aleatoria.
#   4. Regenera JWT_SECRET si no cumple el mínimo de Loco (base64, 64 bytes).
#   5. Si ya había una base local, avisa: conserva la contraseña antigua.
#
# Se puede repetir sin riesgo: no pisa valores ya configurados y válidos.
#
# Requiere en el host: bash, make y Docker.
# -----------------------------------------------------------------------------
source "$(dirname "$0")/_common.sh"

ENV_FILE=docker/.env
EXAMPLE=docker/.env.example
COMPOSE="docker compose -f docker/compose.yml"
VOLUME=inst_user_monitor_pgdata

# Cambia el valor de KEY en docker/.env (o lo añade si no está).
# Se usa `sed -i.bak` + borrar el .bak porque es la única forma de `sed -i`
# que funciona igual en Linux y en macOS.
set_env() {
	local key="$1" value="$2"
	if grep -q "^$key=" "$ENV_FILE"; then
		sed -i.bak "s|^$key=.*|$key=$value|" "$ENV_FILE"
		rm -f "$ENV_FILE.bak"
	else
		echo "$key=$value" >>"$ENV_FILE"
	fi
}

# 48 caracteres hexadecimales aleatorios: sin símbolos que haya que escapar
# (la contraseña de la base va dentro de DATABASE_URL).
random_secret() {
	head -c 24 /dev/urandom | od -An -tx1 | tr -d ' \n'
}

# Secreto de firma de los JWT: Loco firma con HS512, que exige una clave de al
# menos 64 bytes (RFC 7518 §3.2). Va en base64, como lo espera Loco.
JWT_SECRET_BYTES=64
random_jwt_secret() {
	head -c "$JWT_SECRET_BYTES" /dev/urandom | base64 | tr -d '\n'
}

# true si JWT_SECRET en docker/.env es base64 de al menos 64 bytes.
valid_jwt_secret() {
	local value bytes
	value=$(grep -E '^JWT_SECRET=' "$ENV_FILE" | cut -d= -f2-)
	bytes=$(printf '%s' "$value" | base64 -d 2>/dev/null | wc -c) || return 1
	[[ "$bytes" -ge "$JWT_SECRET_BYTES" ]]
}

# --- 1. Host y Docker --------------------------------------------------------
# /.dockerenv solo existe dentro de un contenedor.
if [[ -f /.dockerenv ]]; then
	fail "make setup se ejecuta en el HOST, no dentro del Dev Container"
fi
command -v docker >/dev/null || fail "No se encuentra el comando docker"
docker info >/dev/null 2>&1 || fail "Docker no responde: arráncalo y repite"
ok "Docker disponible"

# --- 2. docker/.env ----------------------------------------------------------
if [[ -f "$ENV_FILE" ]]; then
	info "$ENV_FILE ya existe: se conserva"
else
	cp "$EXAMPLE" "$ENV_FILE"
	ok "Creado $ENV_FILE desde la plantilla"
fi

# --- 3. Contraseñas ----------------------------------------------------------
# Toda variable cuyo valor sea exactamente `change-me` recibe una propia.
changed_db_password=false
for key in $(grep -E '^[A-Z_]+=change-me$' "$ENV_FILE" | cut -d= -f1); do
	if [[ "$key" == JWT_SECRET ]]; then
		set_env "$key" "$(random_jwt_secret)"
	else
		set_env "$key" "$(random_secret)"
	fi
	ok "$key generada"
	if [[ "$key" == POSTGRES_PASSWORD ]]; then
		changed_db_password=true
	fi
done

# --- 4. JWT_SECRET válido -----------------------------------------------------
# Los .env de versiones anteriores tienen un secreto más corto: la API ya no
# arrancaría con él. Los tokens emitidos con el anterior dejan de valer.
if ! valid_jwt_secret; then
	set_env JWT_SECRET "$(random_jwt_secret)"
	ok "JWT_SECRET regenerado (base64 de $JWT_SECRET_BYTES bytes)"
fi

# --- 5. Base local existente -------------------------------------------------
# PostgreSQL solo aplica POSTGRES_PASSWORD al crear el volumen: si el volumen
# ya existe, guarda la contraseña antigua y la API no podría conectarse.
if $changed_db_password && docker volume inspect "$VOLUME" >/dev/null 2>&1; then
	printf '%s⚠ Ya existe una base local (%s) creada con otra contraseña.%s\n' "$YELLOW$BOLD" "$VOLUME" "$RESET"
	confirm "¿Borrarla para que use la nueva? Se pierden sus datos"
	$COMPOSE down
	docker volume rm "$VOLUME"
	ok "Base local borrada: se recreará al abrir el Dev Container"
fi

# --- Siguiente paso ----------------------------------------------------------
echo
ok "Todo listo. Ahora:"
echo "   1. Abre esta carpeta en VS Code"
echo "   2. Elige «Reopen in Container»"
echo "   3. Dentro, ejecuta: make help"
