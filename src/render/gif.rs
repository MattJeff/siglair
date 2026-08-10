//! Rasterisation d'un document en GIF — contrat §7.4 à §7.7.
//!
//! HTML (`html.rs`, mode Freeform, `for_capture`) → Chromium → une image PNG par frame →
//! `ffmpeg` en deux passes palette. Aucune requête sortante n'est nécessaire pour nos
//! propres médias : ils sont écrits dans le dossier temporaire et référencés en `file:`.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use anyhow::anyhow;
use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat;
use chromiumoxide::cdp::js_protocol::runtime::EvaluateParams;
use chromiumoxide::handler::viewport::Viewport;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt as _;
use url::Url;
use uuid::Uuid;

use super::job::RenderError;
use super::{html::render_document, RenderMode, RenderOpts};
use crate::config::Config;
use crate::doc::{AnimPreset, Doc, ElementType, Profile};
use crate::error::AppError;

/// Facteur de rendu : le GIF fait deux fois la taille CSS du canvas, ce qui le rend net
/// sur les écrans à haute densité où sont lus la plupart des e-mails.
const SCALE: f64 = 2.0;
/// Contrat §7.5 : 12 images/s, 10 s au plus, donc 120 frames au maximum.
const MAX_DURATION: f64 = 10.0;
const MAX_FRAMES: u32 = 120;
/// Gmail coupe autour de 1–2 Mo, les passerelles d'entreprise sont plus strictes : au-delà,
/// le GIF n'est pas « moins beau », il est invisible.
const MAX_GIF_BYTES: usize = 1_000_000;
/// Une signature de 6 s à 12 fps demande 72 captures haute densité puis un encodage global.
/// Trente secondes était inférieur au temps normal du CX23 et transformait un rendu sain en
/// trois échecs successifs. Le plafond reste dur pour tuer tout Chromium réellement bloqué.
const JOB_TIMEOUT: Duration = Duration::from_secs(90);

/// Média référencé par le document, déjà lu depuis le `Storage`.
pub struct Asset {
    pub id: Uuid,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub struct Gif {
    pub gif: Vec<u8>,
    /// Première frame — repli pour les clients qui n'animent pas (contrat §5.1).
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub fps: u32,
}

pub async fn render(
    cfg: &Config,
    doc: &Doc,
    profile: &Profile,
    assets: &[Asset],
) -> Result<Gif, RenderError> {
    let chrome = cfg.chrome_path.as_deref().ok_or_else(|| {
        RenderError::msg("Le service de rendu est momentanément indisponible. Réessayez plus tard.")
    })?;

    // Contrat §8 : l'URL est revalidée au rendu et pas seulement à l'enregistrement,
    // un DNS pouvant changer de réponse entre les deux.
    if !doc.canvas.bg_image.is_empty() {
        let raw = doc.canvas.bg_image.clone();
        let checked = tokio::task::spawn_blocking(move || crate::doc::check_remote_url(&raw))
            .await
            .map_err(|e| RenderError::new("Le rendu a échoué. Réessayez dans un instant.", e))?;
        if let Err(e) = checked {
            let why = match &e {
                AppError::Validation(m) => m.clone(),
                _ => "L'image de fond n'a pas pu être téléchargée.".to_string(),
            };
            return Err(RenderError::new(why, e));
        }
    }

    // Dossier de travail propre, supprimé même en cas d'erreur.
    let dir = std::env::temp_dir().join(format!("siglair-render-{}", Uuid::new_v4()));
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| RenderError::new("Le rendu a échoué. Réessayez dans un instant.", e))?;
    let out = run(cfg, chrome, doc, profile, assets, &dir).await;
    let _ = tokio::fs::remove_dir_all(&dir).await;
    out
}

async fn run(
    cfg: &Config,
    chrome: &str,
    doc: &Doc,
    profile: &Profile,
    assets: &[Asset],
    dir: &Path,
) -> Result<Gif, RenderError> {
    let oops =
        |e: std::io::Error| RenderError::new("Le rendu a échoué. Réessayez dans un instant.", e);

    // Nos médias deviennent des fichiers locaux : Chromium n'a plus aucune raison de sortir
    // sur le réseau pour eux, et un rendu ne dépend pas de la joignabilité de notre propre API.
    let asset_dir = dir.join("assets");
    tokio::fs::create_dir_all(&asset_dir).await.map_err(oops)?;
    let mut urls: HashMap<Uuid, String> = HashMap::with_capacity(assets.len());
    for a in assets {
        let path = asset_dir.join(format!("{}.{}", a.id, ext_of(&a.content_type)));
        tokio::fs::write(&path, &a.bytes).await.map_err(oops)?;
        if let Ok(u) = Url::from_file_path(&path) {
            urls.insert(a.id, u.to_string());
        }
    }

    let opts = RenderOpts {
        public_url: cfg.public_url.clone(),
        // Une image ne porte pas de lien : le suivi des clics vit dans le HTML d'export.
        slug: None,
        assets: urls,
        for_capture: true,
        // La mention Siglair s'affiche sous l'image, hors du canvas : la capturer la couperait.
        branding: false,
    };
    let page_path = dir.join("page.html");
    tokio::fs::write(
        &page_path,
        render_document(doc, profile, RenderMode::Freeform, &opts),
    )
    .await
    .map_err(oops)?;
    let page_url = Url::from_file_path(&page_path)
        .map_err(|_| RenderError::msg("Le rendu a échoué. Réessayez dans un instant."))?;

    let (w, h) = (doc.canvas.width_px(), doc.canvas.height_px());
    let mut builder = BrowserConfig::builder()
        .chrome_executable(chrome)
        .user_data_dir(dir.join("chrome"))
        .viewport(Viewport {
            width: w,
            height: h,
            device_scale_factor: Some(SCALE),
            ..Default::default()
        })
        .window_size(w, h)
        .launch_timeout(Duration::from_secs(10))
        .request_timeout(Duration::from_secs(10))
        .disable_cache()
        .args(vec![
            "--disable-dev-shm-usage",
            "--disable-gpu",
            "--hide-scrollbars",
            "--disable-extensions",
            "--disable-background-networking",
            "--no-first-run",
            "--no-default-browser-check",
            // Les médias sont locaux : plus rien de légitime ne pointe vers la machine hôte.
            // Les littéraux IP privés, eux, sont barrés en amont par `doc::check_remote_url`.
            "--host-resolver-rules=MAP localhost ~NOTFOUND,MAP *.localhost ~NOTFOUND,\
             MAP *.internal ~NOTFOUND,MAP *.local ~NOTFOUND",
        ]);
    // Contrat §8 : `--no-sandbox` uniquement quand le conteneur est déjà isolé, donc sur
    // décision explicite de l'exploitant, jamais par défaut.
    if std::env::var("CHROME_NO_SANDBOX").is_ok() {
        builder = builder.no_sandbox();
    }
    let config = builder.build().map_err(|m| {
        RenderError::new(
            "Le service de rendu est mal configuré.",
            anyhow!("chromium: {m}"),
        )
    })?;

    let (mut browser, mut handler) = Browser::launch(config).await.map_err(|e| {
        RenderError::new(
            "Le service de rendu est momentanément indisponible. Réessayez plus tard.",
            e,
        )
    })?;
    let pump = tokio::spawn(async move { while handler.next().await.is_some() {} });

    let result = tokio::time::timeout(
        JOB_TIMEOUT,
        capture_and_encode(&browser, cfg, doc, dir, page_url.as_str()),
    )
    .await;

    // Un Chromium orphelin par job raté remplit la machine en une nuit : on ferme
    // proprement si c'est rapide, on tue sinon, et on récolte le processus dans tous les cas.
    if tokio::time::timeout(Duration::from_secs(3), browser.close())
        .await
        .is_err()
    {
        let _ = browser.kill().await;
    }
    let _ = tokio::time::timeout(Duration::from_secs(5), browser.wait()).await;
    pump.abort();

    match result {
        Ok(out) => out,
        Err(elapsed) => Err(RenderError::new(
            "Le rendu de la signature a pris trop de temps. Simplifiez-la (moins d'éléments, \
             moins d'animations) puis republiez.",
            elapsed,
        )),
    }
}

async fn capture_and_encode(
    browser: &Browser,
    cfg: &Config,
    doc: &Doc,
    dir: &Path,
    page_url: &str,
) -> Result<Gif, RenderError> {
    let boom = |e: chromiumoxide::error::CdpError| {
        RenderError::new(
            "La signature n'a pas pu être affichée pour être capturée.",
            e,
        )
    };
    let page = browser.new_page(page_url).await.map_err(boom)?;
    wait_ready(&page).await?;

    // ponytail : une signature sans animation n'a besoin que d'une frame — GIF minuscule,
    // capture instantanée. Une vidéo n'est pas pilotable par `getAnimations` : on l'ignore.
    let animated = doc
        .elements
        .iter()
        .any(|e| !e.hidden && e.anim.preset != AnimPreset::None && e.kind != ElementType::Video);
    let duration = doc.timeline_duration.clamp(0.1, MAX_DURATION);

    // Contrat §7.7 : trop lourd → 8 img/s, puis 128 couleurs, puis 64.
    let ladder: &[(u32, u32)] = if animated {
        &[(12, 256), (8, 256), (8, 128), (8, 64)]
    } else {
        &[(1, 256), (1, 128), (1, 64)]
    };

    let mut captured_fps = 0u32;
    let mut frames = 0u32;
    let mut gif = Vec::new();

    for &(want_fps, colors) in ladder {
        // Descendre de 12 à 8 images/s se fait en recapturant, pas en jetant des frames :
        // un sous-échantillonnage 12→8 n'est pas entier et se voit à l'œil.
        if captured_fps != want_fps {
            frames = capture(
                &page,
                &frames_dir(dir, want_fps),
                want_fps,
                if animated { duration } else { 0.0 },
            )
            .await?;
            captured_fps = want_fps;
        }
        gif = encode(
            &cfg.ffmpeg_path,
            &frames_dir(dir, want_fps),
            want_fps,
            colors,
            &dir.join("out.gif"),
        )
        .await?;
        if gif.len() <= MAX_GIF_BYTES {
            break;
        }
        tracing::debug!(
            bytes = gif.len(),
            fps = want_fps,
            colors,
            "GIF trop lourd, on descend d'un cran"
        );
    }
    if gif.len() > MAX_GIF_BYTES {
        tracing::warn!(
            bytes = gif.len(),
            "GIF au-dessus de la cible malgré tous les paliers"
        );
    }

    let png = tokio::fs::read(frames_dir(dir, captured_fps).join("frame0000.png"))
        .await
        .map_err(|e| RenderError::new("Le rendu a échoué. Réessayez dans un instant.", e))?;

    Ok(Gif {
        gif,
        png,
        width: (doc.canvas.width_px() as f64 * SCALE) as u32,
        height: (doc.canvas.height_px() as f64 * SCALE) as u32,
        frames,
        fps: captured_fps,
    })
}

fn frames_dir(dir: &Path, fps: u32) -> std::path::PathBuf {
    dir.join(format!("frames-{fps}"))
}

/// Attend les polices et les images avant la frame 0 : capturer une image encore vide
/// donne un GIF qui « clignote » à la première boucle. Plafonné côté page pour qu'une
/// ressource distante muette ne consomme pas le budget de 30 s à elle seule.
const READY_JS: &str = "new Promise(function(res){\
    var done=function(){\
      var waits=Array.prototype.slice.call(document.images).filter(function(i){return !i.complete;})\
        .map(function(i){return new Promise(function(r){i.addEventListener('load',r,{once:true});\
                                                        i.addEventListener('error',r,{once:true});});});\
      waits.push(document.fonts?document.fonts.ready:Promise.resolve());\
      Promise.all(waits).then(function(){res(true);},function(){res(true);});\
    };\
    if(document.readyState==='complete'){done();}else{window.addEventListener('load',done,{once:true});}\
    setTimeout(function(){res(false);},8000);\
  })";

async fn wait_ready(page: &Page) -> Result<(), RenderError> {
    let params = EvaluateParams::builder()
        .expression(READY_JS)
        .await_promise(true)
        .return_by_value(true)
        .build()
        .map_err(|m| RenderError::new("Le rendu a échoué.", anyhow!("evaluate: {m}")))?;
    let ready: bool = page
        .evaluate(params)
        .await
        .map_err(|e| {
            RenderError::new(
                "La signature n'a pas pu être chargée pour être capturée.",
                e,
            )
        })?
        .into_value()
        .unwrap_or(false);
    if !ready {
        // Non bloquant : mieux vaut un GIF avec une image manquante qu'aucun GIF du tout.
        tracing::warn!("chargement des ressources incomplet avant la capture");
    }
    Ok(())
}

/// Capture déterministe : l'horloge de chaque animation est positionnée avant chaque frame.
/// Aucun `sleep` entre les captures — dormir donne un timing irrégulier et un GIF qui saccade.
async fn capture(page: &Page, out: &Path, fps: u32, duration: f64) -> Result<u32, RenderError> {
    tokio::fs::create_dir_all(out)
        .await
        .map_err(|e| RenderError::new("Le rendu a échoué. Réessayez dans un instant.", e))?;
    let n = ((duration * fps as f64).round() as i64).clamp(1, MAX_FRAMES as i64) as u32;

    for i in 0..n {
        let t = (i as f64) * 1000.0 / fps as f64;
        set_clock(page, t).await?;
        let shot = ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .build();
        let png = page.screenshot(shot).await.map_err(|e| {
            RenderError::new(
                "La capture de la signature a échoué. Réessayez dans un instant.",
                e,
            )
        })?;
        tokio::fs::write(out.join(format!("frame{i:04}.png")), png)
            .await
            .map_err(|e| RenderError::new("Le rendu a échoué. Réessayez dans un instant.", e))?;
    }
    Ok(n)
}

async fn set_clock(page: &Page, t_ms: f64) -> Result<(), RenderError> {
    let js = format!(
        "(function(){{var t={t_ms};\
          (document.getAnimations?document.getAnimations():[]).forEach(function(a){{\
            try{{a.pause();a.currentTime=t;}}catch(e){{}}\
          }});return true;}})()"
    );
    let params = EvaluateParams::builder()
        .expression(js)
        .return_by_value(true)
        .build()
        .map_err(|m| RenderError::new("Le rendu a échoué.", anyhow!("evaluate: {m}")))?;
    page.evaluate(params).await.map_err(|e| {
        RenderError::new(
            "Les animations de la signature n'ont pas pu être capturées.",
            e,
        )
    })?;
    Ok(())
}

/// Deux passes `palettegen`/`paletteuse` dans un seul graphe : une palette **globale** donne
/// un GIF nettement plus propre qu'une palette recalculée à chaque image.
async fn encode(
    ffmpeg: &str,
    frames: &Path,
    fps: u32,
    colors: u32,
    out: &Path,
) -> Result<Vec<u8>, RenderError> {
    let _ = tokio::fs::remove_file(out).await;
    let output = tokio::process::Command::new(ffmpeg)
        .args(["-y", "-nostdin", "-hide_banner", "-loglevel", "error"])
        .arg("-framerate")
        .arg(fps.to_string())
        .arg("-i")
        .arg(frames.join("frame%04d.png"))
        .arg("-filter_complex")
        .arg(format!(
            "[0:v]palettegen=max_colors={colors}[p];[0:v][p]paletteuse=dither=bayer"
        ))
        .arg("-loop")
        .arg("0")
        .arg(out)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| {
            RenderError::new(
                "Le service de rendu est momentanément indisponible. Réessayez plus tard.",
                e,
            )
        })?;

    if !output.status.success() {
        // Le détail ffmpeg va dans les logs, jamais dans le message rendu à l'utilisateur.
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(RenderError::new(
            "La conversion de la signature en GIF a échoué. Réessayez dans un instant.",
            anyhow!(
                "ffmpeg ({}) : {}",
                output.status,
                err.chars().take(500).collect::<String>()
            ),
        ));
    }
    tokio::fs::read(out)
        .await
        .map_err(|e| RenderError::new("La conversion de la signature en GIF a échoué.", e))
}

/// Chromium détermine le type d'un `file:` par son extension : sans extension plausible,
/// l'image ne s'affiche pas.
fn ext_of(content_type: &str) -> String {
    let raw = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .rsplit('/')
        .next()
        .unwrap_or("");
    let e: String = raw
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect();
    if e.is_empty() {
        "bin".to_string()
    } else {
        e.to_ascii_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_are_plausible_and_inert() {
        assert_eq!(ext_of("image/png"), "png");
        assert_eq!(ext_of("image/jpeg; charset=binary"), "jpeg");
        assert_eq!(ext_of("video/mp4"), "mp4");
        // rien qui puisse s'échapper d'un nom de fichier
        assert_eq!(ext_of("image/../../etc/passwd"), "passwd");
        assert_eq!(ext_of(""), "bin");
    }
}
