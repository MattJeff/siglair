//! Erreurs de l'API — forme de réponse imposée par le contrat §5.4 :
//! `{ "error": { "code": "...", "message": "..." } }`, message affichable tel quel.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not_found")]
    NotFound,
    #[error("validation: {0}")]
    Validation(String),
    #[error("quota_exceeded: {0}")]
    QuotaExceeded(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("rate_limited")]
    RateLimited,
    /// Hors de la liste du contrat §5.4, ajoutée au plus près : une fonctionnalité dont la
    /// clé tierce est absente (facturation sans `STRIPE_SECRET_KEY`) n'est ni une erreur du
    /// client, ni une panne. Le front doit pouvoir masquer le bouton plutôt que réessayer.
    #[error("not_implemented: {0}")]
    NotImplemented(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn validation(msg: impl Into<String>) -> Self {
        Self::Validation(msg.into())
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Vous devez être connecté pour effectuer cette action.".to_string(),
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "Vous n'avez pas les droits nécessaires sur cette ressource.".to_string(),
            ),
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "Cette ressource n'existe pas ou n'est plus disponible.".to_string(),
            ),
            AppError::Validation(m) => (StatusCode::UNPROCESSABLE_ENTITY, "validation", m),
            AppError::QuotaExceeded(m) => (StatusCode::PAYMENT_REQUIRED, "quota_exceeded", m),
            AppError::Conflict(m) => (StatusCode::CONFLICT, "conflict", m),
            AppError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Trop de tentatives. Réessayez dans quelques minutes.".to_string(),
            ),
            AppError::NotImplemented(m) => (StatusCode::NOT_IMPLEMENTED, "not_implemented", m),
            AppError::Internal(err) => {
                // La cause complète va dans les logs, jamais dans la réponse : un message
                // SQL ou un chemin de fichier renvoyé au client est une fuite d'information.
                tracing::error!(error = ?err, "erreur interne");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "Une erreur interne est survenue. Réessayez dans un instant.".to_string(),
                )
            }
        };
        (
            status,
            Json(json!({ "error": { "code": code, "message": message } })),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            other => AppError::Internal(other.into()),
        }
    }
}

macro_rules! internal_from {
    ($($t:ty),* $(,)?) => {$(
        impl From<$t> for AppError {
            fn from(e: $t) -> Self { AppError::Internal(e.into()) }
        }
    )*};
}

internal_from!(std::io::Error, serde_json::Error, reqwest::Error);
