//! Módulo de autenticación (arquitectura hexagonal):
//!
//! - `domain`: entidad `User`, value objects (`Email`, `Password`) y puertos.
//! - `application`: casos de uso (`AuthService`).
//! - `infrastructure`: adaptadores (PostgreSQL, Argon2, JWT).
//! - `http`: adaptador de entrada (rutas, DTOs, extractor `AuthUser`).
//!
//! Este archivo hace el cableado: elige qué adaptador implementa cada puerto.

pub mod application;
pub mod domain;
