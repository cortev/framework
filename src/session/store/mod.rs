use std::{collections::HashMap, future::Future, time::Duration};

use anyhow::Context;
use axum::{http::StatusCode, response::IntoResponse};
use rand::distributions::{Alphanumeric, DistString};

use super::Session;

pub(crate) type SessionData = HashMap<String, serde_json::Value>;

trait ToJson {
    fn to_json(&self) -> SessionResult<String>;
}

impl ToJson for SessionData {
    fn to_json(&self) -> SessionResult<String> {
        let value = serde_json::to_string(&self)
            .context("failed to serialize session data")?;
        Ok(value)
    }
}

#[cfg(feature = "memory")]
pub mod memory;

type SessionResult<T> = Result<T, SessionError>;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("session was not found")]
    NotFound,
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

impl IntoResponse for SessionError {
    fn into_response(self) -> axum::response::Response {
        match self {
            SessionError::NotFound => {
                (StatusCode::NOT_FOUND, axum::response::Json("session not found")).into_response()
            }
            SessionError::Unexpected(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, axum::response::Json("unexpected error")).into_response()
            }
        }
    }
}

pub trait SessionStore {
    fn read(&self, key: &str) -> impl Future<Output = SessionResult<Session>> + Send;
    fn write(&self, key: String, data: SessionData) -> impl Future<Output = SessionResult<String>> + Send;
    fn destroy(&self, key: &str) -> impl Future<Output = SessionResult<()>> + Send;
    fn ttl(&self) -> Duration;
}

pub trait SessionManager: SessionStore + Sync {
    fn create(&self, data: SessionData) -> impl Future<Output = SessionResult<String>> + Send {
        let key = generate_random_key();
        self.write(key, data)
    }

    fn init(&self) -> impl Future<Output = SessionResult<String>> + Send {
        self.create(SessionData::default())
    }

    fn regenerate(&self, key: &str, data: SessionData) -> impl Future<Output = SessionResult<String>> + Send {
        async move {
            let session_key = self.create(data).await?;
            self.destroy(key).await?;
            Ok(session_key)
        }
    }

    fn invalidate(&self, key: &str) -> impl Future<Output = SessionResult<String>> + Send {
        async move {
            self.destroy(key).await?;
            self.init().await
        }
    }
}

/// Generates a random session key.
///
/// [OWASP recommends](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#session-id-entropy)
pub fn generate_random_key() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 64)
}