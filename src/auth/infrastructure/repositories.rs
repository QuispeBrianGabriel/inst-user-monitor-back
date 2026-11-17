//! Adaptador de `UserRepository` sobre PostgreSQL.

use sqlx::PgPool;
use uuid::Uuid;

use super::entities::UserEntity;
use crate::auth::domain::{
    Email, User,
    ports::{RepositoryError, UserRepository},
};

const UNIQUE_EMAIL: &str = "users_email_key";

#[derive(Clone)]
pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl UserRepository for PostgresUserRepository {
    async fn create(&self, user: &User) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO users (id, email, password_hash, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(user.id())
        .bind(user.email().as_str())
        .bind(user.password_hash().as_str())
        .bind(user.created_at())
        .bind(user.updated_at())
        .execute(&self.pool)
        .await
        .map_err(|error| match error.as_database_error() {
            Some(db) if db.constraint() == Some(UNIQUE_EMAIL) => RepositoryError::EmailTaken,
            _ => RepositoryError::Unexpected(error.into()),
        })?;
        Ok(())
    }

    async fn find_by_email(&self, email: &Email) -> Result<Option<User>, RepositoryError> {
        sqlx::query_as::<_, UserEntity>(
            "SELECT id, email, password_hash, created_at, updated_at
             FROM users WHERE email = $1",
        )
        .bind(email.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| RepositoryError::Unexpected(error.into()))?
        .map(User::try_from)
        .transpose()
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, RepositoryError> {
        sqlx::query_as::<_, UserEntity>(
            "SELECT id, email, password_hash, created_at, updated_at
             FROM users WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| RepositoryError::Unexpected(error.into()))?
        .map(User::try_from)
        .transpose()
    }
}
