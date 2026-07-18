use std::error::Error;

use serde::ser::{Serialize, Serializer};
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("Request canceled.")]
    Cancelled,
    #[error("{}", error_with_sources(.0))]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),
    #[error(transparent)]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Tauri(#[from] tauri::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl AppError {
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

fn error_with_sources(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();

    while let Some(error) = source {
        let source_message = error.to_string();
        if !source_message.is_empty() && !message.contains(&source_message) {
            message.push_str(": ");
            message.push_str(&source_message);
        }
        source = error.source();
    }

    message
}

#[cfg(test)]
mod tests {
    use super::AppError;

    #[test]
    fn stable_domain_errors_serialize_as_strings() {
        assert_eq!(
            serde_json::to_string(&AppError::Message("stable message".to_owned()))
                .expect("serialize message"),
            "\"stable message\""
        );
        assert_eq!(
            serde_json::to_string(&AppError::Cancelled).expect("serialize cancellation"),
            "\"Request canceled.\""
        );
    }

    #[test]
    fn standard_errors_keep_typed_causes() {
        assert!(matches!(
            AppError::from(std::io::Error::other("disk failure")),
            AppError::Io(_)
        ));
        assert!(matches!(
            AppError::from(url::ParseError::EmptyHost),
            AppError::Url(_)
        ));
        assert!(matches!(
            AppError::from(serde_json::from_str::<serde_json::Value>("{").unwrap_err()),
            AppError::Json(_)
        ));
    }
}
