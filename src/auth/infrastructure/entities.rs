//! Filas de la base de datos y su conversión a entidades del dominio.

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth::domain::{Email, PasswordHash, User, ports::RepositoryError};

/// Fila de la tabla `users`.
#[derive(Debug, FromRow)]
pub struct UserEntity {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<UserEntity> for User {
    type Error = RepositoryError;

    fn try_from(row: UserEntity) -> Result<Self, Self::Error> {
        let email = Email::parse(&row.email).map_err(|error| {
            RepositoryError::Unexpected(
                format!("email inválido en users {}: {error}", row.id).into(),
            )
        })?;
        Ok(User::restore(
            row.id,
            email,
            PasswordHash::new(row.password_hash),
            row.created_at,
            row.updated_at,
        ))
    }
}
