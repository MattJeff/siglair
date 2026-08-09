//! Stockage des médias et des rendus. Une seule implémentation aujourd'hui (disque) ;
//! le trait existe parce que S3 viendra le jour où il y a deux machines (contrat §1).

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::error::{AppError, Result};

#[async_trait]
pub trait Storage: Send + Sync {
    async fn put(&self, key: &str, bytes: Vec<u8>, content_type: &str) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Vec<u8>>;
    async fn delete(&self, key: &str) -> Result<()>;
    /// URL publique servant cette clé.
    fn url(&self, key: &str) -> String;
}

pub struct FsStorage {
    root: PathBuf,
    base_url: String,
}

impl FsStorage {
    pub fn new(root: impl Into<PathBuf>, base_url: impl AsRef<str>) -> Self {
        Self {
            root: root.into(),
            base_url: base_url.as_ref().trim_end_matches('/').to_string(),
        }
    }

    fn path(&self, key: &str) -> Result<PathBuf> {
        Ok(self.root.join(safe_key(key)?))
    }
}

#[async_trait]
impl Storage for FsStorage {
    async fn put(&self, key: &str, bytes: Vec<u8>, _content_type: &str) -> Result<()> {
        let path = self.path(key)?;
        if let Some(dir) = path.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        tokio::fs::write(&path, bytes).await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>> {
        match tokio::fs::read(self.path(key)?).await {
            Ok(b) => Ok(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(AppError::NotFound),
            Err(e) => Err(e.into()),
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        match tokio::fs::remove_file(self.path(key)?).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    fn url(&self, key: &str) -> String {
        format!("{}/f/{}", self.base_url, key)
    }
}

/// Une clé est un chemin relatif de caractères anodins. Tout le reste est refusé :
/// une traversée ici (`../../etc/passwd`) donne une lecture arbitraire du disque.
fn safe_key(key: &str) -> Result<&Path> {
    let ok = !key.is_empty()
        && key.len() <= 512
        && !key.starts_with('/')
        && !key
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'));
    if ok {
        Ok(Path::new(key))
    } else {
        Err(AppError::validation("Clé de stockage invalide."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal() {
        for bad in [
            "../etc/passwd",
            "/etc/passwd",
            "a/../../b",
            "",
            "a//b",
            "a\\b",
            "a b",
            "a/..",
            "x$(id)",
        ] {
            assert!(safe_key(bad).is_err(), "aurait dû refuser {bad:?}");
        }
        for good in ["renders/2026/abc.gif", "assets/a-b_c.png"] {
            assert!(safe_key(good).is_ok(), "aurait dû accepter {good:?}");
        }
    }
}
