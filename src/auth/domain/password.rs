//! Value objects `Password` (contraseña en claro, validada) y `PasswordHash`.
//!
//! Política de contraseña segura:
//! - Entre 12 y 128 caracteres. El máximo evita abusar del hash (DoS).
//! - Solo ASCII imprimible (letras, números, símbolos y espacio): sin emojis,
//!   tildes ni caracteres de control, que se escriben distinto según el
//!   teclado o el sistema y dejan al usuario fuera de su cuenta.
//! - Al menos una minúscula, una mayúscula, un número y un símbolo.
//!
//! Ninguno de los dos tipos se imprime en logs: `Debug` muestra `***`.

use std::fmt;

use thiserror::Error;

/// Contraseña en claro que cumple la política. Solo vive en memoria.
#[derive(Clone)]
pub struct Password(String);

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PasswordError {
    #[error("la contraseña debe tener al menos {min} caracteres", min = Password::MIN_LEN)]
    TooShort,
    #[error("la contraseña no puede superar {max} caracteres", max = Password::MAX_LEN)]
    TooLong,
    #[error("la contraseña solo admite letras sin tilde, números, símbolos y espacios")]
    InvalidCharacters,
    #[error("la contraseña debe incluir al menos una minúscula")]
    MissingLowercase,
    #[error("la contraseña debe incluir al menos una mayúscula")]
    MissingUppercase,
    #[error("la contraseña debe incluir al menos un número")]
    MissingDigit,
    #[error("la contraseña debe incluir al menos un símbolo")]
    MissingSymbol,
}

impl Password {
    pub const MIN_LEN: usize = 12;
    pub const MAX_LEN: usize = 128;

    /// Valida una contraseña nueva. No se recorta: los espacios cuentan.
    pub fn parse(raw: &str) -> Result<Self, PasswordError> {
        // Bytes ≥ caracteres: se corta una entrada enorme antes de recorrerla.
        if raw.len() > Self::MAX_LEN {
            return Err(PasswordError::TooLong);
        }
        if !raw.chars().all(|c| c == ' ' || c.is_ascii_graphic()) {
            return Err(PasswordError::InvalidCharacters);
        }
        if raw.len() < Self::MIN_LEN {
            return Err(PasswordError::TooShort);
        }
        if !raw.chars().any(|c| c.is_ascii_lowercase()) {
            return Err(PasswordError::MissingLowercase);
        }
        if !raw.chars().any(|c| c.is_ascii_uppercase()) {
            return Err(PasswordError::MissingUppercase);
        }
        if !raw.chars().any(|c| c.is_ascii_digit()) {
            return Err(PasswordError::MissingDigit);
        }
        if !raw.chars().any(|c| c.is_ascii_punctuation() || c == ' ') {
            return Err(PasswordError::MissingSymbol);
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(***)")
    }
}

/// Hash de una contraseña (formato PHC, p. ej. `$argon2id$v=19$...`).
/// Es lo único que se persiste.
#[derive(Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

impl PasswordHash {
    pub fn new(hash: String) -> Self {
        Self(hash)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PasswordHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PasswordHash(***)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_strong_passwords() {
        for password in [
            "Sup3r-Secret!",
            "correct Horse 7 battery",
            "A1!aaaaaaaaa",
            &format!("Aa1!{}", "x".repeat(124)),
        ] {
            assert!(
                Password::parse(password).is_ok(),
                "{password} debería ser válida"
            );
        }
    }

    #[test]
    fn rejects_weak_passwords() {
        let cases = [
            ("Ab1!short", PasswordError::TooShort),
            ("alllowercase1!", PasswordError::MissingUppercase),
            ("ALLUPPERCASE1!", PasswordError::MissingLowercase),
            ("NoDigitsHere!!", PasswordError::MissingDigit),
            ("NoSymbols1234", PasswordError::MissingSymbol),
            ("Contraseña-123", PasswordError::InvalidCharacters),
            ("Emoji😀-Pass123", PasswordError::InvalidCharacters),
            ("Tab\tSeparated-1A", PasswordError::InvalidCharacters),
        ];
        for (password, expected) in cases {
            assert_eq!(
                Password::parse(password).err(),
                Some(expected),
                "{password}"
            );
        }
    }

    #[test]
    fn rejects_too_long() {
        let password = format!("Aa1!{}", "x".repeat(125));
        assert_eq!(
            Password::parse(&password).err(),
            Some(PasswordError::TooLong)
        );
    }

    #[test]
    fn debug_does_not_leak_secrets() {
        let password = Password::parse("Sup3r-Secret!").unwrap();
        assert_eq!(format!("{password:?}"), "Password(***)");
        let hash = PasswordHash::new("$argon2id$...".into());
        assert_eq!(format!("{hash:?}"), "PasswordHash(***)");
    }
}
