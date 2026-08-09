//! Configuration issue de l'environnement — contrat §9.
//!
//! Seules `DATABASE_URL` et `SIGLAIR_SECRET_KEY` sont obligatoires. L'absence d'une clé
//! tierce ne fait jamais échouer le démarrage : elle désactive la fonctionnalité, et
//! `GET /api/config` le dit au front pour qu'il masque le bouton correspondant.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    /// Origine du front (redirections OAuth, liens des e-mails). Sans slash final.
    pub app_url: String,
    /// Origine publique servie aux clients mail (`{public_url}/s/{slug}.gif`). Sans slash final.
    pub public_url: String,
    pub storage_dir: PathBuf,
    pub port: u16,
    /// 32 octets, signature des cookies.
    pub secret_key: Vec<u8>,
    pub ip_salt: String,
    pub google: Option<GoogleConfig>,
    pub apple: Option<AppleConfig>,
    pub resend: Option<ResendConfig>,
    pub stripe: Option<StripeConfig>,
    pub chrome_path: Option<String>,
    pub ffmpeg_path: String,
}

#[derive(Debug, Clone)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone)]
pub struct AppleConfig {
    pub client_id: String,
    pub team_id: String,
    pub key_id: String,
    /// Contenu PEM du fichier .p8.
    pub private_key: String,
}

#[derive(Debug, Clone)]
pub struct ResendConfig {
    pub api_key: String,
    pub from: String,
}

#[derive(Debug, Clone)]
pub struct StripeConfig {
    pub secret_key: String,
    pub webhook_secret: String,
    pub price_pro: Option<String>,
    pub price_team: Option<String>,
}

/// Ce que renvoie `GET /api/config`.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Features {
    pub google: bool,
    pub apple: bool,
    pub magic: bool,
    pub billing: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();

        let database_url = env("DATABASE_URL").context(
            "DATABASE_URL manquant. Exemple : \
             DATABASE_URL=postgres://siglair:siglair@localhost:5432/siglair",
        )?;

        let secret_raw = env("SIGLAIR_SECRET_KEY")
            .context("SIGLAIR_SECRET_KEY manquant. Générez-la avec : openssl rand -base64 32")?;
        let secret_key = B64.decode(secret_raw.as_bytes()).map_err(|_| {
            anyhow!("SIGLAIR_SECRET_KEY n'est pas du base64 valide. Générez-la avec : openssl rand -base64 32")
        })?;
        if secret_key.len() < 32 {
            return Err(anyhow!(
                "SIGLAIR_SECRET_KEY fait {} octets, il en faut au moins 32. Générez-la avec : openssl rand -base64 32",
                secret_key.len()
            ));
        }

        let app_url = trim_url(env("APP_URL").unwrap_or_else(|| "http://localhost:5173".into()));
        let public_url = trim_url(env("PUBLIC_URL").unwrap_or_else(|| app_url.clone()));

        // Sans sel explicite on en dérive un du secret : les hachages d'IP restent stables
        // pour un déploiement donné sans exiger une variable de plus au premier lancement.
        let ip_salt = env("SIGLAIR_IP_SALT").unwrap_or_else(|| B64.encode(&secret_key));

        Ok(Self {
            database_url,
            app_url,
            public_url,
            storage_dir: env("STORAGE_DIR")
                .unwrap_or_else(|| "./data/storage".into())
                .into(),
            port: env("PORT").and_then(|p| p.parse().ok()).unwrap_or(8080),
            secret_key,
            ip_salt,
            google: both("GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET").map(
                |(client_id, client_secret)| GoogleConfig {
                    client_id,
                    client_secret,
                },
            ),
            apple: apple_from_env(),
            resend: both("RESEND_API_KEY", "RESEND_FROM")
                .map(|(api_key, from)| ResendConfig { api_key, from }),
            stripe: both("STRIPE_SECRET_KEY", "STRIPE_WEBHOOK_SECRET").map(
                |(secret_key, webhook_secret)| StripeConfig {
                    secret_key,
                    webhook_secret,
                    price_pro: env("STRIPE_PRICE_PRO"),
                    price_team: env("STRIPE_PRICE_TEAM"),
                },
            ),
            chrome_path: env("CHROME_PATH"),
            ffmpeg_path: env("FFMPEG_PATH").unwrap_or_else(|| "ffmpeg".into()),
        })
    }

    pub fn features(&self) -> Features {
        Features {
            google: self.google.is_some(),
            apple: self.apple.is_some(),
            magic: self.resend.is_some(),
            billing: self.stripe.is_some(),
        }
    }
}

fn apple_from_env() -> Option<AppleConfig> {
    Some(AppleConfig {
        client_id: env("APPLE_CLIENT_ID")?,
        team_id: env("APPLE_TEAM_ID")?,
        key_id: env("APPLE_KEY_ID")?,
        // Les .p8 collés dans un .env perdent souvent leurs retours à la ligne.
        private_key: env("APPLE_PRIVATE_KEY")?.replace("\\n", "\n"),
    })
}

/// Une variable vide vaut une variable absente : `RESEND_API_KEY=` désactive l'e-mail
/// au lieu de faire échouer chaque appel avec une clé vide.
fn env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn both(a: &str, b: &str) -> Option<(String, String)> {
    Some((env(a)?, env(b)?))
}

fn trim_url(u: String) -> String {
    u.trim_end_matches('/').to_string()
}
