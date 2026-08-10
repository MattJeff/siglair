//! Relais temporaire entre la génération publique et le compte créé par magic link.
//!
//! Le document peut faire plusieurs kilo-octets et ne doit pas voyager dans l'URL. On le
//! dépose donc dans le stockage pendant 24 h ; l'URL ne porte qu'un jeton signé contenant
//! l'identifiant et l'expiration. Aucun schéma SQL supplémentaire n'est nécessaire.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::{
    ai::{Brand, VariantSource},
    doc::{Doc, Profile},
    error::{AppError, Result},
    AppState,
};

const TTL_SECONDS: i64 = 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub signature_id: Uuid,
    pub brand: Brand,
    pub doc: Doc,
    pub profile: Profile,
    pub name: String,
    pub source: VariantSource,
}

#[derive(Serialize, Deserialize)]
struct Claims {
    id: Uuid,
    exp: i64,
}

type HmacSha256 = Hmac<Sha256>;

pub async fn store(st: &AppState, draft: &Draft) -> Result<String> {
    let claims = Claims {
        id: Uuid::new_v4(),
        exp: Utc::now().timestamp() + TTL_SECONDS,
    };
    st.storage
        .put(
            &key(claims.id),
            serde_json::to_vec(draft)?,
            "application/json",
        )
        .await?;
    Ok(seal(&st.cfg.secret_key, &claims))
}

pub async fn load(st: &AppState, token: &str) -> Result<(Uuid, Draft)> {
    let claims = open(&st.cfg.secret_key, token).ok_or_else(expired)?;
    let bytes = st.storage.get(&key(claims.id)).await.map_err(|error| {
        if matches!(error, AppError::NotFound) {
            expired()
        } else {
            error
        }
    })?;
    let draft = serde_json::from_slice(&bytes).map_err(|_| expired())?;
    Ok((claims.id, draft))
}

pub async fn delete(st: &AppState, id: Uuid) {
    if let Err(error) = st.storage.delete(&key(id)).await {
        tracing::warn!(?error, %id, "brouillon d'onboarding non supprimé");
    }
}

/// Vérification légère pour ne pas injecter une valeur arbitraire dans un lien envoyé.
pub fn valid(st: &AppState, token: &str) -> bool {
    open(&st.cfg.secret_key, token).is_some()
}

fn key(id: Uuid) -> String {
    format!("onboarding/drafts/{id}.json")
}

fn expired() -> AppError {
    AppError::validation(
        "Cette création a expiré. Relancez l'analyse de votre site pour préparer une nouvelle signature.",
    )
}

fn seal(secret: &[u8], claims: &Claims) -> String {
    let payload = B64URL.encode(serde_json::to_vec(claims).expect("claims sérialisables"));
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepte toute taille de clé");
    mac.update(payload.as_bytes());
    format!("{payload}.{}", B64URL.encode(mac.finalize().into_bytes()))
}

fn open(secret: &[u8], raw: &str) -> Option<Claims> {
    let (payload, signature) = raw.split_once('.')?;
    if raw.len() > 512 {
        return None;
    }
    let mut mac = HmacSha256::new_from_slice(secret).ok()?;
    mac.update(payload.as_bytes());
    mac.verify_slice(&B64URL.decode(signature).ok()?).ok()?;
    let claims: Claims = serde_json::from_slice(&B64URL.decode(payload).ok()?).ok()?;
    (claims.exp > Utc::now().timestamp()).then_some(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_jeton_modifie_est_refuse() {
        let secret = [7_u8; 32];
        let claims = Claims {
            id: Uuid::new_v4(),
            exp: Utc::now().timestamp() + 60,
        };
        let token = seal(&secret, &claims);
        assert!(open(&secret, &token).is_some());
        let mut forged = token.into_bytes();
        forged[4] = if forged[4] == b'a' { b'b' } else { b'a' };
        assert!(open(&secret, std::str::from_utf8(&forged).unwrap()).is_none());
    }

    #[test]
    fn un_jeton_expire_est_refuse() {
        let secret = [9_u8; 32];
        let token = seal(
            &secret,
            &Claims {
                id: Uuid::new_v4(),
                exp: Utc::now().timestamp() - 1,
            },
        );
        assert!(open(&secret, &token).is_none());
    }
}
