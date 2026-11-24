//! Cuentas de Instagram que cada usuario monitorea a mano, y las capturas
//! (snapshots) de sus listas de seguidores y seguidos que va pegando.
//!
//! `monitored_accounts`
//! - `user_id`: dueño. Cada usuario solo ve sus cuentas; al borrarlo se
//!   borran con él.
//! - `username`: el usuario de Instagram, en minúsculas (lo normaliza el
//!   modelo). Único por dueño: dos usuarios pueden monitorear la misma cuenta.
//! - `full_name`, `notes`: datos libres que añade el usuario.
//!
//! `account_snapshots`
//! - `kind`: qué lista se pegó, `followers` o `following`.
//! - `entries`: las cuentas de la lista, `[{"username", "full_name"}]`, en el
//!   orden en que se pegaron. Una captura se lee y se compara entera (con la
//!   anterior), nunca por partes: no hace falta una tabla por cuenta.
//! - `total`: `jsonb_array_length(entries)`, guardado para listar el
//!   historial sin cargar las listas.
//! - `captured_at`: cuándo se pegó. El índice sirve para pedir las últimas
//!   capturas de cada cuenta y lista.

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r"
CREATE TABLE monitored_accounts (
    id         uuid         PRIMARY KEY DEFAULT uuidv7(),
    user_id    uuid         NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    username   varchar(30)  NOT NULL,
    full_name  varchar(150),
    notes      text,
    created_at timestamptz  NOT NULL DEFAULT now(),
    updated_at timestamptz  NOT NULL DEFAULT now(),

    CONSTRAINT monitored_accounts_user_username_key UNIQUE (user_id, username),
    CONSTRAINT monitored_accounts_username_format CHECK (username ~ '^[a-z0-9._]{1,30}$')
);

CREATE TRIGGER monitored_accounts_set_updated_at
    BEFORE UPDATE ON monitored_accounts
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE account_snapshots (
    id          uuid        PRIMARY KEY DEFAULT uuidv7(),
    account_id  uuid        NOT NULL REFERENCES monitored_accounts (id) ON DELETE CASCADE,
    kind        varchar(16) NOT NULL,
    total       integer     NOT NULL,
    entries     jsonb       NOT NULL,
    captured_at timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT account_snapshots_kind CHECK (kind IN ('followers', 'following')),
    CONSTRAINT account_snapshots_entries_array CHECK (jsonb_typeof(entries) = 'array'),
    CONSTRAINT account_snapshots_total CHECK (total = jsonb_array_length(entries))
);

CREATE INDEX account_snapshots_account_kind_captured_idx
    ON account_snapshots (account_id, kind, captured_at DESC);
";

const DOWN: &str = r"
DROP TABLE account_snapshots;
DROP TABLE monitored_accounts;
";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection().execute_unprepared(UP).await?;
        Ok(())
    }

    async fn down(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.get_connection().execute_unprepared(DOWN).await?;
        Ok(())
    }
}
