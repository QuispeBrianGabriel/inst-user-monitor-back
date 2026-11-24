#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;

mod m20261118_040507_users;
mod m20261118_043016_drop_sqlx_migrations;
mod m20261124_213821_monitored_accounts;
mod m20261124_223535_drop_kind_from_account_snapshots;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261118_040507_users::Migration),
            Box::new(m20261118_043016_drop_sqlx_migrations::Migration),
            Box::new(m20261124_213821_monitored_accounts::Migration),
            Box::new(m20261124_223535_drop_kind_from_account_snapshots::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}
