//! Petites fonctions partagées : hachage, jetons, slugs.

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine as _};
use rand::Rng;
use sha2::{Digest, Sha256};

use crate::doc::{Doc, Profile};

/// Contrat §4.1 : jamais d'IP en clair. sha256(ip || salt) tronqué à 16 octets.
pub fn hash_ip(ip: &str, salt: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(ip.as_bytes());
    h.update(salt.as_bytes());
    h.finalize()[..16].to_vec()
}

/// Famille de client mail devinée. Jamais l'User-Agent brut en base (contrat §4.1).
pub fn ua_family(ua: &str) -> &'static str {
    let ua = ua.to_ascii_lowercase();
    if ua.contains("googleimageproxy") || ua.contains("gmail") || ua.contains("google-read-aloud") {
        "gmail"
    } else if ua.contains("outlook") || ua.contains("microsoft") || ua.contains("msoffice") {
        "outlook"
    } else if ua.contains("apple-mail")
        || ua.contains("applemail")
        || ua.contains("iphone")
        || ua.contains("ipad")
        || ua.contains("macintosh")
    {
        "apple-mail"
    } else {
        "other"
    }
}

/// 32 octets aléatoires en base64url — magic link, invitation, state OAuth.
pub fn random_token() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    B64URL.encode(bytes)
}

/// Seul le haché est stocké : une fuite de la table ne donne aucun jeton utilisable.
pub fn hash_token(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Contrat §8 : plafond en mémoire, par instance. Une clé par sujet à limiter
/// (`publish:{org}`, `upload:{org}`, `checkout:{user}`, `magic:{email}`...). Purge
/// paresseuse à chaque appel : la table ne garde que les fenêtres encore ouvertes.
///
/// ponytail : compteur mémoire, remis à zéro au redéploiement et non partagé entre
/// instances. Le jour où il y en a deux, remplacer le corps par un `count(*)` sur une
/// table PG — la signature de la fonction n'a pas à changer.
pub fn rate_limit(key: &str, max: usize, window: Duration) -> bool {
    static HITS: LazyLock<Mutex<HashMap<String, Vec<Instant>>>> = LazyLock::new(Default::default);
    let mut g = HITS.lock().unwrap_or_else(|e| e.into_inner());
    g.retain(|_, hits| {
        hits.retain(|t| t.elapsed() < window);
        !hits.is_empty()
    });
    let hits = g.entry(key.to_string()).or_default();
    if hits.len() >= max {
        return false;
    }
    hits.push(Instant::now());
    true
}

/// Échappement HTML du texte utilisateur. Une seule implémentation pour l'export (§8),
/// la page rendue par Chromium et les e-mails : deux copies finissent par diverger, et
/// celle qui oublie un caractère devient un XSS stocké distribué par e-mail.
pub fn esc_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.to_lowercase().chars() {
        match c {
            'a'..='z' | '0'..='9' => out.push(c),
            'à'..='å' => out.push('a'),
            'è'..='ë' => out.push('e'),
            'ì'..='ï' => out.push('i'),
            'ò'..='ö' => out.push('o'),
            'ù'..='ü' => out.push('u'),
            'ç' => out.push('c'),
            'ñ' => out.push('n'),
            _ if !out.is_empty() && !out.ends_with('-') => out.push('-'),
            _ => {}
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Alphabet sans caractères ambigus : un slug se lit parfois à voix haute ou se recopie.
const SLUG_ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";

pub fn gen_slug() -> String {
    let mut rng = rand::thread_rng();
    (0..12)
        .map(|_| SLUG_ALPHABET[rng.gen_range(0..SLUG_ALPHABET.len())] as char)
        .collect()
}

/// Empreinte du **rendu** : sha256 du JSON canonique de (doc, profil). Le nom `doc_hash`
/// est celui de la colonne, il désigne l'empreinte du rendu entier.
///
/// Le profil en fait partie parce que les jetons `{{...}}` (§3.1) sont rasterisés dans le
/// GIF : ne hacher que le document ferait resservir par le cache (§7.1) un GIF portant
/// l'ancienne fonction après une modification dans Réglages, indéfiniment.
///
/// Les deux passent par `serde_json::Value`, qui trie ses clés (pas de feature
/// `preserve_order` ici) : deux valeurs identiques donnent le même hash, sinon le cache ne
/// servirait jamais.
pub fn doc_hash(doc: &Doc, profile: &Profile) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(canonical_json(doc));
    h.update(canonical_json(profile));
    h.finalize().to_vec()
}

fn canonical_json<T: serde::Serialize>(v: &T) -> Vec<u8> {
    serde_json::to_value(v)
        .and_then(|v| serde_json::to_vec(&v))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::Element;

    #[test]
    fn hashes_and_slugs() {
        assert_eq!(hash_ip("1.2.3.4", "sel").len(), 16);
        assert_ne!(hash_ip("1.2.3.4", "sel"), hash_ip("1.2.3.5", "sel"));
        assert_ne!(hash_ip("1.2.3.4", "sel"), hash_ip("1.2.3.4", "autre"));
        assert_eq!(hash_token("x").len(), 32);
        assert_eq!(gen_slug().len(), 12);
        assert!(gen_slug().bytes().all(|b| SLUG_ALPHABET.contains(&b)));
        assert_eq!(slugify("Café  de l'Été !"), "cafe-de-l-ete");
        assert_eq!(slugify("Acme Corp"), "acme-corp");
        assert_eq!(
            ua_family("Mozilla/5.0 (via ggpht.com GoogleImageProxy)"),
            "gmail"
        );
        assert_eq!(ua_family("Microsoft Outlook 16.0"), "outlook");
        assert_eq!(ua_family("curl/8"), "other");
    }

    #[test]
    fn rate_limit_stops_at_the_ceiling_and_isolates_keys() {
        let win = Duration::from_secs(60);
        assert!((0..3).all(|_| rate_limit("test:a", 3, win)));
        assert!(
            !rate_limit("test:a", 3, win),
            "le 4e appel doit être refusé"
        );
        // une clé ne consomme pas le quota d'une autre
        assert!(rate_limit("test:b", 3, win));
        // fenêtre écoulée : le compteur repart
        assert!(rate_limit("test:c", 1, Duration::from_nanos(1)));
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(rate_limit("test:c", 1, Duration::from_nanos(1)));
    }

    #[test]
    fn doc_hash_is_stable_across_field_order() {
        let a: Doc =
            serde_json::from_str(r#"{"v":1,"canvas":{"width":620,"height":250}}"#).unwrap();
        let b: Doc =
            serde_json::from_str(r#"{"canvas":{"height":250,"width":620},"v":1}"#).unwrap();
        let p = Profile::from([("role".into(), "CTO".into()), ("name".into(), "Ada".into())]);
        let p_rev = Profile::from([("name".into(), "Ada".into()), ("role".into(), "CTO".into())]);
        assert_eq!(doc_hash(&a, &p), doc_hash(&b, &p_rev));

        let mut c = a.clone();
        c.elements.push(Element {
            id: "aaa1111".into(),
            ..Default::default()
        });
        assert_ne!(doc_hash(&a, &p), doc_hash(&c, &p));
    }

    /// Le bug corrigé par la migration 0002 : republier après avoir changé sa fonction
    /// dans Réglages doit invalider le cache de rendu, pas resservir l'ancien GIF.
    #[test]
    fn doc_hash_changes_with_the_profile() {
        let doc = Doc::default();
        let before = Profile::from([("role".into(), "CTO".into())]);
        let after = Profile::from([("role".into(), "CEO".into())]);
        assert_ne!(doc_hash(&doc, &before), doc_hash(&doc, &after));
        assert_ne!(doc_hash(&doc, &before), doc_hash(&doc, &Profile::new()));
        // pas de collision par concaténation : déplacer une valeur d'une clé à l'autre
        assert_ne!(
            doc_hash(&doc, &Profile::from([("a".into(), "xy".into())])),
            doc_hash(
                &doc,
                &Profile::from([("a".into(), "x".into()), ("b".into(), "y".into())])
            ),
        );
    }
}
