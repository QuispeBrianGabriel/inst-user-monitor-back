//! Casos de uso de autenticación: registro, login y usuario actual.
//! Solo depende de los puertos del dominio.

use thiserror::Error;
use uuid::Uuid;

use crate::auth::domain::{
    Email, EmailError, Password, PasswordError, User,
    ports::{AccessToken, BoxError, PasswordHasher, RepositoryError, TokenService, UserRepository},
};

#[derive(Debug, Error)]
pub enum AuthError {
    #[error(transparent)]
    InvalidEmail(#[from] EmailError),
    #[error(transparent)]
    WeakPassword(#[from] PasswordError),
    #[error("el email ya está registrado")]
    EmailTaken,
    /// Email o contraseña incorrectos. Mismo error en ambos casos a propósito.
    #[error("credenciales inválidas")]
    InvalidCredentials,
    #[error("token inválido o caducado")]
    InvalidToken,
    #[error(transparent)]
    Unexpected(BoxError),
}

impl From<RepositoryError> for AuthError {
    fn from(error: RepositoryError) -> Self {
        match error {
            RepositoryError::EmailTaken => Self::EmailTaken,
            RepositoryError::Unexpected(error) => Self::Unexpected(error),
        }
    }
}

pub struct AuthService<R, H, T> {
    users: R,
    hasher: H,
    tokens: T,
}

impl<R, H, T> AuthService<R, H, T>
where
    R: UserRepository,
    H: PasswordHasher,
    T: TokenService,
{
    pub fn new(users: R, hasher: H, tokens: T) -> Self {
        Self {
            users,
            hasher,
            tokens,
        }
    }

    /// Crea un usuario con el email y la contraseña (hasheada).
    pub async fn register(&self, email: &str, password: &str) -> Result<User, AuthError> {
        let email = Email::parse(email)?;
        let password = Password::parse(password)?;

        let hash = self
            .hasher
            .hash(&password)
            .await
            .map_err(AuthError::Unexpected)?;
        let user = User::register(email, hash);
        // La unicidad la garantiza la base (sin carreras entre comprobar e insertar).
        self.users.create(&user).await?;
        Ok(user)
    }

    /// Comprueba las credenciales y emite un token de acceso.
    pub async fn login(&self, email: &str, password: &str) -> Result<AccessToken, AuthError> {
        // En el login no se aplica la política de contraseñas (puede cambiar
        // y dejar fuera a usuarios antiguos); solo el límite de tamaño.
        let Ok(email) = Email::parse(email) else {
            return Err(AuthError::InvalidCredentials);
        };
        if password.len() > Password::MAX_LEN {
            return Err(AuthError::InvalidCredentials);
        }

        let user = self.users.find_by_email(&email).await?;
        let hash = user
            .as_ref()
            .map_or(self.hasher.dummy_hash(), User::password_hash);
        let matches = self
            .hasher
            .verify(password, hash)
            .await
            .map_err(AuthError::Unexpected)?;

        match user {
            Some(user) if matches => self.tokens.issue(user.id()).map_err(AuthError::Unexpected),
            _ => Err(AuthError::InvalidCredentials),
        }
    }

    /// Id del usuario dueño del token.
    pub fn authenticate(&self, token: &str) -> Result<Uuid, AuthError> {
        self.tokens.verify(token).ok_or(AuthError::InvalidToken)
    }

    /// Usuario autenticado. Si ya no existe, el token deja de valer.
    pub async fn current_user(&self, id: Uuid) -> Result<User, AuthError> {
        self.users
            .find_by_id(id)
            .await?
            .ok_or(AuthError::InvalidToken)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::auth::domain::PasswordHash;

    const PASSWORD: &str = "Sup3r-Secret!";

    #[derive(Default)]
    struct InMemoryUsers(Mutex<Vec<User>>);

    impl UserRepository for InMemoryUsers {
        async fn create(&self, user: &User) -> Result<(), RepositoryError> {
            let mut users = self.0.lock().unwrap();
            if users.iter().any(|u| u.email() == user.email()) {
                return Err(RepositoryError::EmailTaken);
            }
            users.push(user.clone());
            Ok(())
        }

        async fn find_by_email(&self, email: &Email) -> Result<Option<User>, RepositoryError> {
            let users = self.0.lock().unwrap();
            Ok(users.iter().find(|u| u.email() == email).cloned())
        }

        async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, RepositoryError> {
            let users = self.0.lock().unwrap();
            Ok(users.iter().find(|u| u.id() == id).cloned())
        }
    }

    /// "Hash" reversible: suficiente para probar el flujo, no la criptografía.
    struct FakeHasher(PasswordHash);

    impl PasswordHasher for FakeHasher {
        async fn hash(&self, password: &Password) -> Result<PasswordHash, BoxError> {
            Ok(PasswordHash::new(format!("hashed:{}", password.expose())))
        }

        async fn verify(&self, candidate: &str, hash: &PasswordHash) -> Result<bool, BoxError> {
            Ok(hash.as_str() == format!("hashed:{candidate}"))
        }

        fn dummy_hash(&self) -> &PasswordHash {
            &self.0
        }
    }

    struct FakeTokens;

    impl TokenService for FakeTokens {
        fn issue(&self, user_id: Uuid) -> Result<AccessToken, BoxError> {
            Ok(AccessToken {
                token: user_id.to_string(),
                expires_in: 900,
            })
        }

        fn verify(&self, token: &str) -> Option<Uuid> {
            token.parse().ok()
        }
    }

    fn service() -> AuthService<InMemoryUsers, FakeHasher, FakeTokens> {
        let dummy = PasswordHash::new("hashed:dummy".into());
        AuthService::new(InMemoryUsers::default(), FakeHasher(dummy), FakeTokens)
    }

    #[tokio::test]
    async fn register_stores_normalized_email_and_hash() {
        let service = service();
        let user = service.register(" John@Gmail.com", PASSWORD).await.unwrap();

        assert_eq!(user.email().as_str(), "john@gmail.com");
        assert_ne!(user.password_hash().as_str(), PASSWORD);
        assert_eq!(user.created_at(), user.updated_at());
    }

    #[tokio::test]
    async fn register_rejects_invalid_input() {
        let service = service();
        assert!(matches!(
            service.register("not-an-email", PASSWORD).await,
            Err(AuthError::InvalidEmail(_))
        ));
        assert!(matches!(
            service.register("john@gmail.com", "weak").await,
            Err(AuthError::WeakPassword(_))
        ));
    }

    #[tokio::test]
    async fn register_rejects_duplicate_email_ignoring_case() {
        let service = service();
        service.register("john@gmail.com", PASSWORD).await.unwrap();
        assert!(matches!(
            service.register("JOHN@gmail.com", PASSWORD).await,
            Err(AuthError::EmailTaken)
        ));
    }

    #[tokio::test]
    async fn login_issues_token_for_valid_credentials() {
        let service = service();
        let user = service.register("john@gmail.com", PASSWORD).await.unwrap();

        let token = service.login("JOHN@gmail.com", PASSWORD).await.unwrap();
        let id = service.authenticate(&token.token).unwrap();
        assert_eq!(id, user.id());
        assert_eq!(service.current_user(id).await.unwrap().id(), user.id());
    }

    #[tokio::test]
    async fn login_fails_the_same_way_for_wrong_password_or_unknown_email() {
        let service = service();
        service.register("john@gmail.com", PASSWORD).await.unwrap();

        for (email, password) in [
            ("john@gmail.com", "Wrong-Pass123"),
            ("nobody@gmail.com", PASSWORD),
            ("not-an-email", PASSWORD),
            ("john@gmail.com", &"A".repeat(Password::MAX_LEN + 1)),
        ] {
            assert!(matches!(
                service.login(email, password).await,
                Err(AuthError::InvalidCredentials)
            ));
        }
    }

    #[tokio::test]
    async fn authenticate_rejects_invalid_token() {
        assert!(matches!(
            service().authenticate("garbage"),
            Err(AuthError::InvalidToken)
        ));
    }
}
