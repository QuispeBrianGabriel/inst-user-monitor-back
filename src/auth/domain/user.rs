//! Entidad `User`: identidad + credenciales.

use chrono::{DateTime, SubsecRound, Utc};
use uuid::Uuid;

use super::{Email, PasswordHash};

#[derive(Debug, Clone)]
pub struct User {
    id: Uuid,
    email: Email,
    password_hash: PasswordHash,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl User {
    /// Usuario nuevo: genera el id (UUID v7) y las fechas.
    pub fn register(email: Email, password_hash: PasswordHash) -> Self {
        // PostgreSQL guarda microsegundos: así lo guardado y lo devuelto coinciden.
        let now = Utc::now().trunc_subsecs(6);
        Self {
            id: Uuid::now_v7(),
            email,
            password_hash,
            created_at: now,
            updated_at: now,
        }
    }

    /// Reconstruye un usuario ya persistido.
    pub fn restore(
        id: Uuid,
        email: Email,
        password_hash: PasswordHash,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            email,
            password_hash,
            created_at,
            updated_at,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn email(&self) -> &Email {
        &self.email
    }

    pub fn password_hash(&self) -> &PasswordHash {
        &self.password_hash
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
}
