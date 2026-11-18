//! Borra `_sqlx_migrations`, el registro del migrador de SQLx que usaban las
//! versiones ≤ 0.3.0. Desde la migración de `users` (que adopta esas bases)
//! las migraciones las registra SeaORM en `seaql_migrations`.
//!
//! Sin vuelta atrás: tras aplicarla, una imagen ≤ 0.3.0 intentaría volver a
//! crear `users` y no arrancaría. `down` no hace nada: el registro de SQLx
//! no tiene sentido sin SQLx.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        m.drop_table(
            Table::drop()
                .table("_sqlx_migrations")
                .if_exists()
                .to_owned(),
        )
        .await
    }

    async fn down(&self, _m: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
