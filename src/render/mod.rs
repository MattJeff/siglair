//! Rendu. Le HTML est écrit **une seule fois**, ici, en Rust (contrat §2) : export
//! utilisateur, page chargée par Chromium et aperçu de l'éditeur sortent tous d'`html.rs`.

pub mod gif;
pub mod html;
pub mod job;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderMode {
    /// Un `<img>` vers le GIF hébergé. C'est ce qu'on vend.
    Hosted,
    /// Positionnement absolu + animations CSS. Sert aussi de page source à Chromium.
    #[default]
    Freeform,
    /// Tables imbriquées, aucune animation. Le mode qui survit à Outlook 2016.
    Safe,
}

#[derive(Debug, Clone, Default)]
pub struct RenderOpts {
    /// Origine publique, sans slash final (`https://siglair.app`).
    pub public_url: String,
    /// Slug de la signature publiée : active le GIF hébergé et le suivi des clics
    /// via `/c/{slug}/{element_id}`. `None` = aperçu, liens directs.
    pub slug: Option<String>,
    /// `asset_id` → URL publique du média. À remplir par l'appelant depuis la table `assets`.
    pub assets: HashMap<Uuid, String>,
    /// Page destinée à la capture Chromium : taille exacte, pas de scrollbar, fond opaque.
    pub for_capture: bool,
    /// Plan Free : marque Siglair imposée dans l'export (contrat §6).
    pub branding: bool,
}
