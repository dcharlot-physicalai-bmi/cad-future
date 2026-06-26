//! Authentication: user registration, login, JWT token management.

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::password_hash::SaltString;
use argon2::password_hash::rand_core::OsRng;
use chrono::{Utc, Duration};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

/// JWT claims embedded in each token.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// User ID.
    pub sub: String,
    /// Username.
    pub username: String,
    /// Expiration (Unix timestamp).
    pub exp: usize,
    /// Issued at.
    pub iat: usize,
}

/// Stored user record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    /// Argon2 password hash.
    pub password_hash: String,
    pub created_at: chrono::DateTime<Utc>,
}

/// In-memory user store (swap for database in production).
#[derive(Debug, Clone)]
pub struct UserStore {
    users: Arc<RwLock<HashMap<String, User>>>,
    jwt_secret: String,
}

/// Request payloads.
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: String,
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct AuthError {
    pub error: String,
}

impl UserStore {
    pub fn new(jwt_secret: &str) -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
            jwt_secret: jwt_secret.to_string(),
        }
    }

    /// Register a new user. Returns JWT token on success.
    pub fn register(&self, req: &RegisterRequest) -> Result<AuthResponse, String> {
        let users = self.users.read().map_err(|_| "Lock poisoned")?;
        if users.values().any(|u| u.username == req.username) {
            return Err("Username already taken".into());
        }
        if users.values().any(|u| u.email == req.email) {
            return Err("Email already registered".into());
        }
        drop(users);

        if req.password.len() < 8 {
            return Err("Password must be at least 8 characters".into());
        }

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2.hash_password(req.password.as_bytes(), &salt)
            .map_err(|e| format!("Hash error: {e}"))?
            .to_string();

        let user = User {
            id: Uuid::new_v4().to_string(),
            username: req.username.clone(),
            email: req.email.clone(),
            password_hash: hash,
            created_at: Utc::now(),
        };

        let token = self.create_token(&user)?;
        let resp = AuthResponse {
            token,
            user_id: user.id.clone(),
            username: user.username.clone(),
        };

        let mut users = self.users.write().map_err(|_| "Lock poisoned")?;
        users.insert(user.id.clone(), user);

        Ok(resp)
    }

    /// Login with username + password. Returns JWT token on success.
    pub fn login(&self, req: &LoginRequest) -> Result<AuthResponse, String> {
        let users = self.users.read().map_err(|_| "Lock poisoned")?;
        let user = users.values()
            .find(|u| u.username == req.username)
            .ok_or("Invalid username or password")?;

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|_| "Invalid stored hash")?;
        Argon2::default()
            .verify_password(req.password.as_bytes(), &parsed_hash)
            .map_err(|_| "Invalid username or password".to_string())?;

        let token = self.create_token(user)?;
        Ok(AuthResponse {
            token,
            user_id: user.id.clone(),
            username: user.username.clone(),
        })
    }

    /// Validate a JWT token and return the claims.
    pub fn validate_token(&self, token: &str) -> Result<Claims, String> {
        let data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &Validation::default(),
        ).map_err(|e| format!("Invalid token: {e}"))?;
        Ok(data.claims)
    }

    fn create_token(&self, user: &User) -> Result<String, String> {
        let now = Utc::now();
        let exp = now + Duration::hours(24);
        let claims = Claims {
            sub: user.id.clone(),
            username: user.username.clone(),
            exp: exp.timestamp() as usize,
            iat: now.timestamp() as usize,
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        ).map_err(|e| format!("Token error: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> UserStore {
        UserStore::new("test-secret-key-for-jwt-signing")
    }

    #[test]
    fn register_and_login() {
        let s = store();
        let resp = s.register(&RegisterRequest {
            username: "alice".into(),
            email: "alice@example.com".into(),
            password: "password123".into(),
        }).unwrap();
        assert_eq!(resp.username, "alice");
        assert!(!resp.token.is_empty());

        // Login
        let resp2 = s.login(&LoginRequest {
            username: "alice".into(),
            password: "password123".into(),
        }).unwrap();
        assert_eq!(resp2.username, "alice");
    }

    #[test]
    fn duplicate_username_rejected() {
        let s = store();
        s.register(&RegisterRequest {
            username: "bob".into(),
            email: "bob@example.com".into(),
            password: "password123".into(),
        }).unwrap();

        let err = s.register(&RegisterRequest {
            username: "bob".into(),
            email: "bob2@example.com".into(),
            password: "password456".into(),
        }).unwrap_err();
        assert!(err.contains("already taken"));
    }

    #[test]
    fn wrong_password_rejected() {
        let s = store();
        s.register(&RegisterRequest {
            username: "carol".into(),
            email: "carol@example.com".into(),
            password: "password123".into(),
        }).unwrap();

        let err = s.login(&LoginRequest {
            username: "carol".into(),
            password: "wrongpassword".into(),
        }).unwrap_err();
        assert!(err.contains("Invalid"));
    }

    #[test]
    fn token_validation() {
        let s = store();
        let resp = s.register(&RegisterRequest {
            username: "dave".into(),
            email: "dave@example.com".into(),
            password: "password123".into(),
        }).unwrap();

        let claims = s.validate_token(&resp.token).unwrap();
        assert_eq!(claims.username, "dave");
        assert_eq!(claims.sub, resp.user_id);
    }

    #[test]
    fn invalid_token_rejected() {
        let s = store();
        assert!(s.validate_token("garbage.token.here").is_err());
    }

    #[test]
    fn short_password_rejected() {
        let s = store();
        let err = s.register(&RegisterRequest {
            username: "eve".into(),
            email: "eve@example.com".into(),
            password: "short".into(),
        }).unwrap_err();
        assert!(err.contains("8 characters"));
    }
}
