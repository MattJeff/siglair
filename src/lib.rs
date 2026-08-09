//! Siglair — bibliothèque partagée par les binaires `api` et `renderer`.
//! Contrat : docs/CONTRACT.md.

pub mod config;
pub mod db;
pub mod doc;
pub mod error;
pub mod plans;
pub mod render;
pub mod storage;
pub mod util;

pub mod ai;
pub mod auth;
pub mod billing;
pub mod email;
pub mod routes;

use std::sync::Arc;

use sqlx::PgPool;

use crate::{config::Config, storage::Storage};

/// État partagé par tous les handlers axum. Cloner ne clone que l'`Arc`.
#[derive(Clone)]
pub struct AppState(Arc<StateInner>);

pub struct StateInner {
    pub db: PgPool,
    pub cfg: Arc<Config>,
    pub storage: Arc<dyn Storage>,
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(db: PgPool, cfg: Config, storage: Arc<dyn Storage>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("siglair/0.1")
            .build()
            .expect("client HTTP");
        Self(Arc::new(StateInner {
            db,
            cfg: Arc::new(cfg),
            storage,
            http,
        }))
    }
}

// Permet `state.db`, `state.cfg.public_url`, `state.http` sans passer par `.0`.
impl std::ops::Deref for AppState {
    type Target = StateInner;
    fn deref(&self) -> &StateInner {
        &self.0
    }
}
