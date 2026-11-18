//! Usuarios que se autentican en la API.
//!
//! - `id`: UUID v7 (ordenable por fecha de creación). Lo genera el modelo al
//!   insertar; el DEFAULT solo cubre inserciones manuales.
//! - `email`: siempre en minúsculas (lo normaliza el modelo), así la
//!   restricción UNIQUE no distingue mayúsculas. 254 es el máximo del RFC.
//! - `password_hash`: hash Argon2id en formato PHC. Nunca la contraseña.
//! - `updated_at`: lo mantiene un trigger en cada UPDATE, venga de donde venga.
//!   La función usa `CREATE OR REPLACE`: `db reset` y la config de test
//!   borran las tablas, pero no las funciones.
//!
//! Hasta la v0.3.0 las migraciones las aplicaba SQLx (tabla
//! `_sqlx_migrations`), con este mismo esquema. En esas bases la tabla ya
//! existe: la migración la adopta tal cual en lugar de crearla.

use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, Statement},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r"
CREATE TABLE users (
    id            uuid         PRIMARY KEY DEFAULT uuidv7(),
    email         varchar(254) NOT NULL,
    password_hash text         NOT NULL,
    created_at    timestamptz  NOT NULL DEFAULT now(),
    updated_at    timestamptz  NOT NULL DEFAULT now(),

    CONSTRAINT users_email_key UNIQUE (email),
    CONSTRAINT users_email_lowercase CHECK (email = lower(email))
);

CREATE OR REPLACE FUNCTION set_updated_at() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

CREATE TRIGGER users_set_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();
";

const DOWN: &str = r"
DROP TABLE users;
DROP FUNCTION set_updated_at();
";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        if created_by_sqlx(m).await? {
            return Ok(());
        }
        m.get_connection().execute_unprepared(UP).await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection().execute_unprepared(DOWN).await?;
        Ok(())
    }
}

/// `true` si SQLx ya aplicó su migración 0001 (`create users`), que crea
/// exactamente este esquema.
async fn created_by_sqlx(m: &SchemaManager<'_>) -> Result<bool, DbErr> {
    if !m.has_table("_sqlx_migrations").await? {
        return Ok(false);
    }
    let db = m.get_connection();
    let applied = db
        .query_one_raw(Statement::from_string(
            db.get_database_backend(),
            "SELECT 1 FROM _sqlx_migrations WHERE version = 1 AND success",
        ))
        .await?;
    Ok(applied.is_some())
}
