//! User authentication models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents an authenticated user account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(default = "Uuid::new_v4")]
    pub id: Uuid,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub password_hash: String,
    #[serde(default = "Utc::now", with = "crate::datetime_compat")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now", with = "crate::datetime_compat")]
    pub updated_at: DateTime<Utc>,
    #[serde(default = "default_role")]
    pub role: String,
}

fn default_role() -> String {
    "user".to_string()
}

impl User {
    /// Create a new user.
    pub fn new(username: String, password_hash: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            username,
            email: None,
            password_hash,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            role: "user".to_string(),
        }
    }

    /// Update the updated_at timestamp.
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_new() {
        let user = User::new("alice".to_string(), "hashed_pw".to_string());
        assert_eq!(user.username, "alice");
        assert_eq!(user.role, "user");
        assert!(user.email.is_none());
    }

    #[test]
    fn test_user_roundtrip() {
        let user = User::new("bob".to_string(), "hash123".to_string());
        let json = serde_json::to_string(&user).unwrap();
        let deserialized: User = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.username, "bob");
        assert_eq!(deserialized.id, user.id);
    }

    #[test]
    fn test_user_touch() {
        let mut user = User::new("carol".to_string(), "hash".to_string());
        let before = user.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        user.touch();
        assert!(user.updated_at >= before);
    }
}
