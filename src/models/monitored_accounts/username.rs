//! Nombre de usuario de Instagram: validación y normalización.
//!
//! Las reglas son las de Instagram (no se consulta ninguna API: la cuenta
//! no se verifica, solo su forma):
//!
//! - De 1 a 30 caracteres: letras minúsculas sin tilde, números, `.` y `_`.
//! - No empieza ni termina en punto, ni tiene dos puntos seguidos.
//!
//! Instagram no distingue mayúsculas y siempre los muestra en minúsculas, así
//! que al registrar una cuenta se recorta, se quita la `@` inicial y se pasa a
//! minúsculas. El texto pegado, en cambio, se compara tal cual (ver
//! [`Username::is_valid`]): una línea con mayúsculas es un nombre, no un usuario.

use std::fmt;

/// Usuario de Instagram válido y normalizado.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Username(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UsernameError {
    #[error("el usuario de Instagram es obligatorio")]
    Empty,
    #[error("el usuario de Instagram no puede superar {max} caracteres", max = Username::MAX_LEN)]
    TooLong,
    #[error(
        "el usuario de Instagram solo admite letras sin tilde, números, puntos y guiones bajos, \
         y no puede empezar ni terminar en punto ni tener dos puntos seguidos"
    )]
    InvalidFormat,
}

impl Username {
    pub const MAX_LEN: usize = 30;

    /// Normaliza (recorta, quita la `@` inicial y pasa a minúsculas) y valida
    /// un usuario escrito a mano.
    ///
    /// # Errors
    ///
    /// Cuando está vacío, es demasiado largo o tiene caracteres no permitidos.
    pub fn parse(raw: &str) -> Result<Self, UsernameError> {
        let username = raw.trim();
        let username = username.strip_prefix('@').unwrap_or(username);
        if username.is_empty() {
            return Err(UsernameError::Empty);
        }
        if username.chars().count() > Self::MAX_LEN {
            return Err(UsernameError::TooLong);
        }
        let username = username.to_lowercase();
        if !Self::is_valid(&username) {
            return Err(UsernameError::InvalidFormat);
        }
        Ok(Self(username))
    }

    /// `true` si `text` es, tal cual, un usuario de Instagram: sin normalizar.
    #[must_use]
    pub fn is_valid(text: &str) -> bool {
        (1..=Self::MAX_LEN).contains(&text.len())
            && text
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'_')
            && !text.starts_with('.')
            && !text.ends_with('.')
            && !text.contains("..")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<Username> for String {
    fn from(username: Username) -> Self {
        username.0
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_normalizes() {
        for (raw, expected) in [
            ("duki", "duki"),
            ("  @ElRubiusWTF ", "elrubiuswtf"),
            ("_.josh_ortiz_", "_.josh_ortiz_"),
            ("l4sov.jpg", "l4sov.jpg"),
            (&"a".repeat(30), &"a".repeat(30)),
        ] {
            assert_eq!(Username::parse(raw).unwrap().as_str(), expected, "{raw}");
        }
    }

    #[test]
    fn parse_rejects_invalid() {
        assert_eq!(Username::parse("  "), Err(UsernameError::Empty));
        assert_eq!(Username::parse("@"), Err(UsernameError::Empty));
        assert_eq!(
            Username::parse(&"a".repeat(31)),
            Err(UsernameError::TooLong)
        );
        for raw in [
            ".duki", "duki.", "du..ki", "du ki", "dukí", "du-ki", "du@ki",
        ] {
            assert_eq!(
                Username::parse(raw),
                Err(UsernameError::InvalidFormat),
                "{raw}"
            );
        }
    }

    #[test]
    fn is_valid_does_not_normalize() {
        assert!(Username::is_valid("creativehub.py"));
        assert!(Username::is_valid("5202"));
        for text in [
            "Creative Hub",
            "Duki",
            "@duki",
            " duki",
            "Rocío Terán",
            "0.001%",
        ] {
            assert!(!Username::is_valid(text), "{text}");
        }
    }
}
