# =============================================================================
#  Inst User Monitor (back) — interfaz del desarrollador
#
#  Ejecuta `make` o `make help` para ver todas las tareas.
#  La lógica vive en scripts/: aquí solo se llaman comandos.
#
#  Todas las tareas se ejecutan dentro del Dev Container, salvo `setup`,
#  que se ejecuta en el host antes de abrirlo por primera vez.
#
#  Al añadir una tarea, añade también su línea en `help`.
# =============================================================================

# bash y no sh (dash): con un solo comando, bash se reemplaza por él. dash se
# queda en medio y, tras un Ctrl+C, se mata con SIGINT aunque el comando haya
# terminado bien, y make imprime "*** Interrupt" (ver scripts/graceful.sh).
SHELL         := bash
.DEFAULT_GOAL := help
MAKEFLAGS     += --no-print-directory

# Envoltorio de cierre ordenado con Ctrl+C (ver scripts/graceful.sh)
GRACEFUL := scripts/graceful.sh
CARGO    := $(GRACEFUL) "🛑 Tarea de cargo interrumpida" cargo

# Parámetros opcionales (se pasan como `make tarea VAR=valor`)
T    ?=
NAME ?=
FILE ?=
V    ?=
DB   ?=
URL  ?= http://localhost:8080

# Consola SQL contra la base local, con la misma variable que usa la API
PSQL := psql "$(DATABASE_URL)"

.PHONY: setup help tree secret \
        run dev stop health smoke \
        check test lint format format-check \
        build clean \
        psql db-tables db-migrations migration db-dump db-restore db-reset \
        version release

# =============================================================================
#  📚 Ayuda
# =============================================================================

# Formatos de printf usados por `help`:
#   TITLE → título de grupo
#   ITEM  → tarea + descripción
#   ARGS  → tarea + descripción + parámetros (atenuados)
TITLE := \n\033[1;33m%s\033[0m\n
ITEM  := "  \033[1;36m%-15s\033[0m %s\n"
ARGS  := "  \033[1;36m%-15s\033[0m %s \033[2m%s\033[0m\n"

help:
	@printf "\n\033[1;35m  📡 Inst User Monitor (back)\033[0m \033[2m— make <tarea> [VAR=valor]\033[0m\n"

	@printf "$(TITLE)" "🖥️  Host — se ejecuta FUERA del Dev Container"
	@printf $(ITEM) "setup"         "🧰 Prepara el repo recién clonado (una sola vez)"

	@printf "$(TITLE)" "📚 General"
	@printf $(ITEM) "help"          "📖 Muestra esta ayuda"
	@printf $(ITEM) "tree"          "🌳 Árbol del código del proyecto"
	@printf $(ITEM) "secret"        "🔑 Genera una contraseña aleatoria"

	@printf "$(TITLE)" "🚀 Aplicación"
	@printf $(ITEM) "run"           "▶️  Arranca la API en :8080"
	@printf $(ITEM) "dev"           "🔁 Arranca la API y la reinicia al guardar"
	@printf $(ITEM) "stop"          "⏹️  Detiene la API y libera el 8080"
	@printf $(ARGS) "health"        "💓 Consulta /health" "[URL=]"
	@printf $(ARGS) "smoke"         "🔥 Comprueba health y 404" "[URL=]"

	@printf "$(TITLE)" "🧪 Calidad"
	@printf $(ITEM) "check"         "✅ Lo mismo que el pipeline (format, lint, test, build)"
	@printf $(ARGS) "test"          "🧪 Ejecuta los tests" "[T=filtro]"
	@printf $(ITEM) "lint"          "🔎 Clippy con los warnings como errores"
	@printf $(ITEM) "format"        "🎨 Corrige el formato"
	@printf $(ITEM) "format-check"  "🔍 Verifica el formato sin tocar nada"

	@printf "$(TITLE)" "🏗️  Build"
	@printf $(ITEM) "build"         "📦 Compila el binario release en target/release/"
	@printf $(ITEM) "clean"         "🧹 Borra target/"

	@printf "$(TITLE)" "🐘 Base de datos (local)"
	@printf $(ITEM) "psql"          "💻 Consola SQL"
	@printf $(ITEM) "db-tables"     "📋 Lista las tablas"
	@printf $(ITEM) "db-migrations" "🗂️  Estado de las migraciones"
	@printf $(ARGS) "migration"     "✍️  Crea la siguiente migración" "NAME="
	@printf $(ARGS) "db-dump"       "💾 Copia de seguridad en dumps/" "[DB=url]"
	@printf $(ARGS) "db-restore"    "⏪ Restaura una copia (pide confirmar)" "FILE= [DB=url]"
	@printf $(ITEM) "db-reset"      "💣 Vacía la base (pide confirmar)"

	@printf "$(TITLE)" "🏷️  Release"
	@printf $(ITEM) "version"       "🔖 Última versión publicada"
	@printf $(ARGS) "release"       "🚢 Crea y sube el tag (pide confirmar)" "V=X.Y.Z"
	@printf "\n"

.DEFAULT:
	@printf "\033[1;31m✘\033[0m La tarea '%s' no existe. Usa \033[1;36mmake help\033[0m para ver todas.\n" "$@"

# Único comando que se ejecuta en el host
setup:
	@scripts/setup.sh

tree:
	@$(GRACEFUL) "🛑 Cancelado" tree -a --dirsfirst -I '.git|target|dumps|.github|scripts|docker|.devcontainer|.vscode'

secret:
	@$(GRACEFUL) "🛑 Cancelado" openssl rand -hex 24

# =============================================================================
#  🚀 Aplicación
# =============================================================================

run:
	@$(GRACEFUL) "⏹️  API detenida correctamente" cargo run

dev:
	@$(GRACEFUL) "⏹️  API detenida correctamente" cargo watch -x run

stop:
	@scripts/stop.sh

health:
	@$(GRACEFUL) "🛑 Consulta cancelada" curl -sS -w '\n' $(URL)/health

smoke:
	@scripts/smoke.sh "$(URL)"

# =============================================================================
#  🧪 Calidad
# =============================================================================

check:
	@scripts/check.sh

# Con T se filtran los tests por nombre: make test T=health
test:
	@$(CARGO) test --locked $(T)

lint:
	@$(CARGO) clippy --locked --all-targets -- -D warnings

format:
	@$(CARGO) fmt

format-check:
	@$(CARGO) fmt --check

# =============================================================================
#  🏗️ Build
# =============================================================================

build:
	@$(CARGO) build --locked --release

clean:
	@$(CARGO) clean

# =============================================================================
#  🐘 Base de datos (local)
# =============================================================================

# Mensaje vacío: en psql, Ctrl+C cancela la consulta, no cierra la sesión
psql:
	@$(GRACEFUL) "" $(PSQL)

db-tables:
	@$(GRACEFUL) "🛑 Consulta cancelada" $(PSQL) -c '\dt'

db-migrations:
	@$(GRACEFUL) "🛑 Consulta cancelada" sqlx migrate info

migration:
	@scripts/new-migration.sh "$(NAME)"

# Con DB se apunta a otra base (ej. producción): make db-dump DB='postgres://…'
db-dump:
	@scripts/db-dump.sh "$(DB)"

db-restore:
	@scripts/db-restore.sh "$(FILE)" "$(DB)"

db-reset:
	@scripts/db-reset.sh

# =============================================================================
#  🏷️ Release
# =============================================================================

version:
	@$(GRACEFUL) "🛑 Cancelado" git describe --tags --abbrev=0

release:
	@scripts/release.sh "$(V)"
