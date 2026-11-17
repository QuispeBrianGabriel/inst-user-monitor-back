//! Dominio de autenticación: entidades, value objects y puertos.
//! No depende de HTTP, de la base de datos ni de librerías de criptografía.

mod email;
mod password;
pub mod ports;
mod user;

pub use email::{Email, EmailError};
pub use password::{Password, PasswordError, PasswordHash};
pub use user::User;
