//! E-mails transactionnels via Brevo (contrat §1).
//!
//! Sans `BREVO_API_KEY`, l'envoi n'échoue pas : le message part dans les logs. Le produit
//! doit démarrer et se tester sans clé tierce (contrat §9) — et un lien de connexion visible
//! dans `docker compose logs` est exactement ce qu'il faut en développement.
//!
//! Le même compte Brevo porte le marketing (campagnes, listes). `sync_contact` y pousse les
//! inscrits ; c'est la seule chose que ce fichier fait qui ne soit pas un envoi.

use anyhow::anyhow;
use serde_json::json;

use crate::{
    error::{AppError, Result},
    util::esc_html as esc,
    AppState,
};

const BG: &str = "#070a10";
const SURFACE: &str = "#0e1522";
const ACCENT: &str = "#3b6cff";
const TEXT: &str = "#e8edf7";
const MUTED: &str = "#8b97ad";

pub async fn send_magic_link(st: &AppState, to: &str, link: &str, ttl_minutes: i32) -> Result<()> {
    send(
        st,
        to,
        "Votre lien de connexion Siglair",
        "Connexion à Siglair",
        &[
            "Cliquez sur le bouton ci-dessous pour vous connecter. Aucun mot de passe à retenir."
                .to_string(),
            format!(
                "Ce lien expire dans {ttl_minutes} minutes et ne fonctionne qu'une seule fois. \
                 Si vous n'êtes pas à l'origine de cette demande, ignorez cet e-mail."
            ),
        ],
        Some(("Se connecter", link)),
    )
    .await
}

pub async fn send_invite(
    st: &AppState,
    to: &str,
    org_name: &str,
    inviter: &str,
    link: &str,
) -> Result<()> {
    send(
        st,
        to,
        &format!("Rejoignez {org_name} sur Siglair"),
        "Invitation à une organisation",
        &[
            format!(
                "{} vous invite à rejoindre l'organisation « {} » sur Siglair, \
                 où vous pourrez créer et publier vos signatures e-mail animées.",
                esc(inviter),
                esc(org_name)
            ),
            "Cette invitation est personnelle et expire dans 7 jours.".to_string(),
        ],
        Some(("Rejoindre l'organisation", link)),
    )
    .await
}

pub async fn send_render_failed(
    st: &AppState,
    to: &str,
    signature_name: &str,
    reason: &str,
    link: &str,
) -> Result<()> {
    send(
        st,
        to,
        "Échec de publication de votre signature",
        "La publication n'a pas abouti",
        &[
            format!(
                "Le rendu de votre signature « {} » a échoué. Votre signature précédente \
                 reste en ligne : rien n'est cassé côté destinataires.",
                esc(signature_name)
            ),
            format!("Raison : {}", esc(reason)),
            "Ouvrez l'éditeur pour republier. Si le problème persiste, répondez à cet e-mail."
                .to_string(),
        ],
        Some(("Ouvrir l'éditeur", link)),
    )
    .await
}

// ---------------------------------------------------------------- envoi

/// `pub(crate)` pour `billing::lifecycle` : les relances d'impayé sont du texte de
/// facturation, il vit avec la règle de facturation, pas ici.
pub(crate) async fn send(
    st: &AppState,
    to: &str,
    subject: &str,
    heading: &str,
    paragraphs: &[String],
    cta: Option<(&str, &str)>,
) -> Result<()> {
    let html = html_body(heading, paragraphs, cta);
    let text = text_body(heading, paragraphs, cta);

    let Some(cfg) = st.cfg.brevo.as_ref() else {
        // Pas de clé : mode développement. Le lien est dans le log, c'est voulu.
        tracing::warn!(%to, %subject, body = %text, "BREVO_API_KEY absente — e-mail non envoyé");
        return Ok(());
    };

    let (from_name, from_email) = addr(&cfg.from);
    let mut sender = json!({ "email": from_email });
    if let Some(n) = from_name {
        sender["name"] = json!(n);
    }

    let resp = st
        .http
        .post("https://api.brevo.com/v3/smtp/email")
        .header("api-key", &cfg.api_key)
        .json(&json!({
            "sender": sender,
            // Brevo veut une adresse nue par destinataire ; `to` peut être un
            // « Nom <adresse> » (cf. billing::lifecycle, qui renvoie sur notre propre boîte).
            "to": [{ "email": addr(to).1 }],
            "subject": subject,
            "htmlContent": html,
            // Sans partie texte, la plupart des filtres classent le message en spam.
            "textContent": text,
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        tracing::error!(%status, %detail, "Brevo a refusé l'envoi");
        return Err(AppError::Internal(anyhow!("Brevo a répondu {status}")));
    }
    Ok(())
}

/// Sépare `Nom <adresse@exemple.fr>` en `(Some("Nom"), "adresse@exemple.fr")`. Une adresse
/// nue renvoie `(None, elle-même)`.
fn addr(s: &str) -> (Option<&str>, &str) {
    match (s.rfind('<'), s.rfind('>')) {
        (Some(o), Some(c)) if o < c => {
            let name = s[..o].trim().trim_matches('"').trim();
            (
                (!name.is_empty()).then_some(name),
                s[o + 1..c].trim(),
            )
        }
        _ => (None, s.trim()),
    }
}

// ---------------------------------------------------------------- contacts

/// Pousse un inscrit dans la liste marketing Brevo (`BREVO_LIST_ID`).
///
/// Ne renvoie jamais d'erreur, comme `growth::record` : une synchronisation marketing qui
/// fait échouer une inscription transforme un outil de croissance en panne de connexion.
/// `updateEnabled` rend l'appel idempotent — un contact déjà connu est mis à jour.
pub async fn sync_contact(st: &AppState, email: &str, name: Option<&str>) {
    let Some(cfg) = st.cfg.brevo.as_ref() else {
        return;
    };
    let Some(list_id) = cfg.list_id else {
        return;
    };

    let body = json!({
        "email": email,
        "listIds": [list_id],
        "updateEnabled": true,
        "attributes": { "PRENOM": name.unwrap_or("") },
    });

    match st
        .http
        .post("https://api.brevo.com/v3/contacts")
        .header("api-key", &cfg.api_key)
        .json(&body)
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => {}
        Ok(r) => {
            let status = r.status();
            let detail = r.text().await.unwrap_or_default();
            tracing::warn!(%status, %detail, "Brevo a refusé le contact");
        }
        Err(e) => tracing::warn!(error = %e, "contact Brevo non synchronisé"),
    }
}

// ---------------------------------------------------------------- gabarit

/// Tables et styles en ligne : les clients mail ignorent `<style>`, la grille et le flex.
fn html_body(heading: &str, paragraphs: &[String], cta: Option<(&str, &str)>) -> String {
    let paras = paragraphs
        .iter()
        .map(|p| {
            format!(
                "<p style=\"margin:0 0 16px;font-size:15px;line-height:1.6;color:{TEXT}\">{p}</p>"
            )
        })
        .collect::<String>();

    let button = cta
        .map(|(label, href)| {
            format!(
                "<table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" style=\"margin:8px 0 4px\"><tr><td \
                 style=\"background:{ACCENT};border-radius:10px\"><a href=\"{href}\" \
                 style=\"display:inline-block;padding:13px 26px;font-size:15px;font-weight:600;\
                 color:#ffffff;text-decoration:none\">{label}</a></td></tr></table>\
                 <p style=\"margin:16px 0 0;font-size:12px;line-height:1.5;color:{MUTED};word-break:break-all\">\
                 Si le bouton ne fonctionne pas, copiez ce lien :<br>{href}</p>",
                href = esc(href),
                label = esc(label)
            )
        })
        .unwrap_or_default();

    format!(
        "<!doctype html><html lang=\"fr\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <meta name=\"color-scheme\" content=\"dark light\"><title>{heading}</title></head>\
         <body style=\"margin:0;padding:0;background:{BG}\">\
         <table role=\"presentation\" width=\"100%\" cellpadding=\"0\" cellspacing=\"0\" \
         style=\"background:{BG};padding:32px 16px\"><tr><td align=\"center\">\
         <table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" width=\"100%\" \
         style=\"max-width:520px;background:{SURFACE};border:1px solid #1b2537;border-radius:16px\">\
         <tr><td style=\"padding:32px\">\
         <p style=\"margin:0 0 24px;font-size:17px;font-weight:700;letter-spacing:-0.01em;color:{ACCENT}\">Siglair</p>\
         <h1 style=\"margin:0 0 16px;font-size:21px;line-height:1.3;font-weight:700;color:{TEXT}\">{heading}</h1>\
         {paras}{button}\
         </td></tr></table>\
         <p style=\"margin:20px 0 0;font-size:12px;color:{MUTED}\">Siglair — signatures e-mail animées</p>\
         </td></tr></table></body></html>",
        heading = esc(heading)
    )
}

fn text_body(heading: &str, paragraphs: &[String], cta: Option<(&str, &str)>) -> String {
    let mut out = format!("{heading}\n\n");
    for p in paragraphs {
        out.push_str(&unesc(p));
        out.push_str("\n\n");
    }
    if let Some((label, href)) = cta {
        out.push_str(&format!("{label} : {href}\n\n"));
    }
    out.push_str("— Siglair, signatures e-mail animées");
    out
}

/// Les paragraphes arrivent déjà échappés (ils contiennent du texte utilisateur) ; la
/// version texte doit les rendre lisibles.
fn unesc(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_content_cannot_inject_markup() {
        let html = html_body(
            "Titre",
            &[format!("Bonjour {}", esc("<script>alert(1)</script>"))],
            Some(("Ouvrir", "https://siglair.com/app?a=1&b=2")),
        );
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("https://siglair.com/app?a=1&amp;b=2"));
    }

    #[test]
    fn sender_is_split_for_brevo() {
        assert_eq!(
            addr("Siglair <bonjour@siglair.app>"),
            (Some("Siglair"), "bonjour@siglair.app")
        );
        assert_eq!(addr("\"Siglair\" <bonjour@siglair.app>"), (Some("Siglair"), "bonjour@siglair.app"));
        assert_eq!(addr("bonjour@siglair.app"), (None, "bonjour@siglair.app"));
        assert_eq!(addr(" <bonjour@siglair.app> "), (None, "bonjour@siglair.app"));
    }

    #[test]
    fn text_version_is_readable() {
        let txt = text_body(
            "Titre",
            &[esc("Acme & Co")],
            Some(("Ouvrir", "https://x.test")),
        );
        assert!(txt.contains("Acme & Co"));
        assert!(txt.contains("https://x.test"));
    }
}
