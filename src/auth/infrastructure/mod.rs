//! Adaptadores: implementan los puertos del dominio con tecnologías concretas.

mod entities;
mod password_hasher;
mod repositories;
mod token_service;

pub use password_hasher::Argon2PasswordHasher;
pub use repositories::PostgresUserRepository;
pub use token_service::JwtTokenService;
