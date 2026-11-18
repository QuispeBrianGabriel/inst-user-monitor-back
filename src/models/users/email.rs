//! Email de un usuario: validación y normalización.
//!
//! El RFC 5321/5322 admite direcciones que ningún proveedor real usa
//! (comillas, comentarios, IPs literales, caracteres raros, Unicode…).
//! Aquí se aplica el subconjunto que se acepta en producción hoy:
//!
//! - Solo ASCII, sin espacios. Se recorta y se pasa a minúsculas.
//! - Máximo 254 caracteres en total y 64 en la parte local.
//! - Parte local: letras, números y `. _ + -`; empieza por letra o número,
//!   no termina en punto y no tiene puntos seguidos.
//! - Dominio: al menos dos etiquetas (`gmail.com`), cada una de 1 a 63
//!   caracteres de letras, números y guiones, sin guion al inicio ni al final.
//!   El TLD tiene 2 o más letras (o es punycode `xn--`).

use std::fmt;

use validator::ValidationError;

/// Email válido y normalizado: es lo único que se guarda en `users.email`.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Email(String);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EmailError {
    #[error("el email es obligatorio")]
    Empty,
    #[error("el email no puede superar {max} caracteres", max = Email::MAX_LEN)]
    TooLong,
    #[error("el email no tiene un formato válido")]
    InvalidFormat,
}

impl Email {
    pub const MAX_LEN: usize = 254;
    const MAX_LOCAL_LEN: usize = 64;
    const MAX_LABEL_LEN: usize = 63;

    /// Valida y normaliza (recorta y pasa a minúsculas) un email.
    ///
    /// # Errors
    ///
    /// Cuando el email está vacío, es demasiado largo o no tiene un formato válido.
    pub fn parse(raw: &str) -> Result<Self, EmailError> {
        let email = raw.trim();
        if email.is_empty() {
            return Err(EmailError::Empty);
        }
        // Antes que nada: no se procesa una entrada enorme.
        if email.len() > Self::MAX_LEN {
            return Err(EmailError::TooLong);
        }
        if !email.is_ascii() {
            return Err(EmailError::InvalidFormat);
        }

        let email = email.to_ascii_lowercase();
        let (local, domain) = email.split_once('@').ok_or(EmailError::InvalidFormat)?;
        if !is_valid_local(local) || !is_valid_domain(domain) {
            return Err(EmailError::InvalidFormat);
        }
        Ok(Self(email))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<Email> for String {
    fn from(email: Email) -> Self {
        email.0
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Email({})", self.0)
    }
}

/// Regla de `Validatable`: lo guardado es un email válido y ya normalizado.
pub(super) fn validate_normalized(email: &str) -> Result<(), ValidationError> {
    match Email::parse(email) {
        Ok(parsed) if parsed.as_str() == email => Ok(()),
        Ok(_) => Err(ValidationError::new("email_not_normalized")),
        Err(error) => {
            Err(ValidationError::new("invalid_email").with_message(error.to_string().into()))
        }
    }
}

fn is_valid_local(local: &str) -> bool {
    local.len() <= Email::MAX_LOCAL_LEN
        && local.starts_with(|c: char| c.is_ascii_alphanumeric())
        && !local.ends_with('.')
        && !local.contains("..")
        && local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

fn is_valid_domain(domain: &str) -> bool {
    let labels: Vec<&str> = domain.split('.').collect();
    if labels.len() < 2 || !labels.iter().all(|label| is_valid_label(label)) {
        return false;
    }
    let tld = labels[labels.len() - 1];
    tld.starts_with("xn--") || (tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()))
}

fn is_valid_label(label: &str) -> bool {
    (1..=Email::MAX_LABEL_LEN).contains(&label.len())
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_provider_emails() {
        for email in [
            "john.doe@gmail.com",
            "jane_doe@hotmail.com",
            "user+tag@outlook.com",
            "first-last@yahoo.com.ar",
            "a1@icloud.com",
            "contacto@mi-empresa.com.pe",
            "x@proton.me",
            "user@xn--80ak6aa92e.xn--p1ai",
        ] {
            assert!(Email::parse(email).is_ok(), "{email} debería ser válido");
        }
    }

    #[test]
    fn normalizes_case_and_whitespace() {
        let email = Email::parse("  John.Doe@GMAIL.com ").unwrap();
        assert_eq!(email.as_str(), "john.doe@gmail.com");
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(Email::parse("   "), Err(EmailError::Empty));
    }

    #[test]
    fn rejects_invalid_formats() {
        for email in [
            "plainaddress",
            "@gmail.com",
            "user@",
            "user@gmail",
            "user@@gmail.com",
            "us@er@gmail.com",
            ".user@gmail.com",
            "user.@gmail.com",
            "us..er@gmail.com",
            "user name@gmail.com",
            "\"user\"@gmail.com",
            "user!#$%@gmail.com",
            "user@-gmail.com",
            "user@gmail-.com",
            "user@gmail..com",
            "user@gmail.c",
            "user@gmail.c0m",
            "user@[127.0.0.1]",
            "usér@gmail.com",
            "user😀@gmail.com",
            "user@gmäil.com",
        ] {
            assert_eq!(
                Email::parse(email),
                Err(EmailError::InvalidFormat),
                "{email} debería ser inválido"
            );
        }
    }

    #[test]
    fn rejects_too_long() {
        let local = "a".repeat(65);
        assert_eq!(
            Email::parse(&format!("{local}@gmail.com")),
            Err(EmailError::InvalidFormat)
        );

        let domain = format!("{}.com", vec!["a".repeat(60); 5].join("."));
        assert_eq!(
            Email::parse(&format!("user@{domain}")),
            Err(EmailError::TooLong)
        );
    }

    #[test]
    fn validator_only_accepts_normalized_emails() {
        assert!(validate_normalized("john@gmail.com").is_ok());
        assert!(validate_normalized("John@gmail.com").is_err());
        assert!(validate_normalized("not-an-email").is_err());
    }
}
