#![allow(elided_lifetimes_in_paths)]
#![allow(clippy::wildcard_imports)]
pub use sea_orm_migration::prelude::*;

mod m20261118_040507_users;
mod m20261118_043016_drop_sqlx_migrations;
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20261118_040507_users::Migration),
            Box::new(m20261118_043016_drop_sqlx_migrations::Migration),
            // inject-above (do not remove this comment)
        ]
    }
}
