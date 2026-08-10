//! Worker de rendu — contrat §7. Réclame un job, le rend, recommence.
//!
//! Rien ne l'expose au réseau : il ne parle qu'à PostgreSQL, au disque, à Chromium et à
//! ffmpeg. On peut donc en lancer plusieurs, ou aucun, sans toucher à l'API.

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use siglair::{config::Config, db, render::job, storage::FsStorage, AppState};
use tokio::sync::watch;

/// Sur le CX23, deux captures Chromium 2x en parallèle se ralentissent jusqu'au timeout.
/// La concurrence reste configurable pour une machine plus grande.
const DEFAULT_CONCURRENCY: usize = 1;
const IDLE_SLEEP: Duration = Duration::from_secs(2);

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,siglair=debug".into()),
        )
        .init();

    let cfg = Config::from_env()?;
    // Un worker qui tourne sans savoir rendre est pire qu'un worker absent : il vide la file
    // en marquant tout en échec.
    preflight(&cfg).await;

    let pool = db::connect(&cfg.database_url).await?;
    db::run_migrations(&pool).await?;

    let storage = Arc::new(FsStorage::new(cfg.storage_dir.clone(), &cfg.public_url));
    let state = AppState::new(pool, cfg, storage);

    match job::requeue_stale(&state.db).await {
        Ok(n) if n > 0 => tracing::warn!(jobs = n, "jobs 'running' orphelins remis en file"),
        Ok(_) => {}
        Err(e) => tracing::error!(error = ?e, "remise en file des jobs orphelins impossible"),
    }

    let (tx, rx) = watch::channel(false);
    tokio::spawn(async move {
        wait_for_signal().await;
        tracing::info!("arrêt demandé : le job en cours sera terminé avant de rendre la main");
        let _ = tx.send(true);
    });

    let concurrency = std::env::var("RENDER_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_CONCURRENCY)
        .clamp(1, 8);
    tracing::info!(concurrency, "worker de rendu démarré");

    let workers: Vec<_> = (0..concurrency)
        .map(|_| {
            let (state, shutdown) = (state.clone(), rx.clone());
            tokio::spawn(worker(state, shutdown))
        })
        .collect();
    for w in workers {
        let _ = w.await;
    }
    tracing::info!("worker de rendu arrêté");
    Ok(())
}

/// Confine le garde de lecture du `watch` : le tenir à travers un `await` rendrait le
/// futur non-`Send`.
fn stopping(shutdown: &watch::Receiver<bool>) -> bool {
    *shutdown.borrow()
}

async fn worker(state: AppState, mut shutdown: watch::Receiver<bool>) {
    while !stopping(&shutdown) {
        match job::claim(&state.db).await {
            // On ne réclame plus rien après un SIGTERM, mais un job déjà réclamé va au bout :
            // l'abandonner le laisserait 'running' pendant cinq minutes.
            Ok(Some(j)) => job::run(&state, j).await,
            Ok(None) => idle(&mut shutdown).await,
            Err(e) => {
                tracing::error!(error = ?e, "réclamation d'un job impossible");
                idle(&mut shutdown).await;
            }
        }
    }
}

async fn idle(shutdown: &mut watch::Receiver<bool>) {
    tokio::select! {
        _ = tokio::time::sleep(IDLE_SLEEP) => {}
        _ = shutdown.changed() => {}
    }
}

#[cfg(unix)]
async fn wait_for_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = ?e, "SIGTERM non écoutable");
            return;
        }
    };
    tokio::select! {
        _ = term.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
}

#[cfg(not(unix))]
async fn wait_for_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

/// Vérifie Chromium et ffmpeg au démarrage. En cas d'absence : comment les installer,
/// puis sortie avec un code non nul.
async fn preflight(cfg: &Config) {
    let Some(chrome) = cfg.chrome_path.as_deref() else {
        die(
            "CHROME_PATH n'est pas défini : le worker ne peut produire aucun GIF.\n\
             Installez Chromium puis indiquez le chemin du binaire :\n  \
               Debian/Ubuntu : apt-get install -y chromium && export CHROME_PATH=/usr/bin/chromium\n  \
               Alpine        : apk add chromium && export CHROME_PATH=/usr/bin/chromium-browser\n  \
               macOS         : brew install --cask google-chrome && \
             export CHROME_PATH=\"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome\"",
        );
    };
    if !tokio::fs::try_exists(chrome).await.unwrap_or(false) {
        die(format!(
            "CHROME_PATH pointe vers « {chrome} », qui n'existe pas.\n\
             Vérifiez le chemin, par exemple avec : which chromium || which google-chrome"
        ));
    }

    let ffmpeg = tokio::process::Command::new(&cfg.ffmpeg_path)
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;
    if !matches!(ffmpeg, Ok(s) if s.success()) {
        die(format!(
            "ffmpeg est introuvable ou inutilisable (FFMPEG_PATH = « {} »).\n\
             Installez-le puis, si besoin, renseignez FFMPEG_PATH :\n  \
               Debian/Ubuntu : apt-get install -y ffmpeg\n  \
               Alpine        : apk add ffmpeg\n  \
               macOS         : brew install ffmpeg",
            cfg.ffmpeg_path
        ));
    }
}

fn die(message: impl AsRef<str>) -> ! {
    eprintln!("{}", message.as_ref());
    std::process::exit(1);
}
