//! Las capturas son siempre de la lista de seguidores: se quita `kind`.
//!
//! La v1.2.0 aceptaba también la lista de seguidos con el mismo formato, sin
//! un ejemplo real que lo respaldara. Esas capturas se borran: ya no se
//! muestran en ningún sitio. `down` vuelve a crear la columna con todas las
//! capturas como seguidores (las borradas no vuelven).

use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

const UP: &str = r"
DELETE FROM account_snapshots WHERE kind <> 'followers';
DROP INDEX account_snapshots_account_kind_captured_idx;
ALTER TABLE account_snapshots DROP COLUMN kind;
CREATE INDEX account_snapshots_account_captured_idx
    ON account_snapshots (account_id, captured_at DESC);
";

const DOWN: &str = r"
DROP INDEX account_snapshots_account_captured_idx;
ALTER TABLE account_snapshots
    ADD COLUMN kind varchar(16) NOT NULL DEFAULT 'followers'
    CONSTRAINT account_snapshots_kind CHECK (kind IN ('followers', 'following'));
ALTER TABLE account_snapshots ALTER COLUMN kind DROP DEFAULT;
CREATE INDEX account_snapshots_account_kind_captured_idx
    ON account_snapshots (account_id, kind, captured_at DESC);
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
