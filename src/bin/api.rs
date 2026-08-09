//! Binaire API. Assemble le routeur du contrat §5 et le sert.

use std::{io::IsTerminal, net::SocketAddr, path::Path, sync::Arc, time::Duration};

use axum::{
    extract::{DefaultBodyLimit, Request},
    http::StatusCode,
    middleware::{from_fn, Next},
    response::{IntoResponse, Response},
    Json, Router,
};
use serde_json::json;
use tower_http::{
    compression::CompressionLayer,
    limit::RequestBodyLimitLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use siglair::{auth, billing, config::Config, db, routes, storage::FsStorage, AppState};

/// 10 Mo d'asset (contrat §5.3) plus l'enveloppe multipart.
const BODY_LIMIT: usize = 12 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const SPA_DIR: &str = "web/dist";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let cfg = Config::from_env()?;
    let pool = db::connect(&cfg.database_url).await?;
    // migrations au démarrage : un déploiement, pas deux (contrat §1)
    db::run_migrations(&pool).await?;

    let storage = Arc::new(FsStorage::new(
        cfg.storage_dir.clone(),
        cfg.public_url.clone(),
    ));
    let port = cfg.port;
    let state = AppState::new(pool, cfg, storage);

    let app = Router::new()
        .merge(routes::public::router()) // /s /c /f /health /ready
        .merge(routes::meta::router()) // /api/config
        .merge(auth::router()) // /api/auth/* /api/me
        .merge(billing::webhook_router()) // /api/stripe/webhook — hors session
        .nest("/api", routes::router())
        .with_state(state);

    // les couches viennent après le fallback SPA pour que le front construit soit
    // compressé et tracé lui aussi
    let app = with_spa(app)
        .layer(from_fn(timeout))
        .layer(CompressionLayer::new())
        // l'ordre compte : désactiver la limite d'axum, puis poser la nôtre par-dessus
        .layer(DefaultBodyLimit::disable())
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).await?;
    tracing::info!(port, "siglair api");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

/// Front construit servi par le même binaire quand il existe : un conteneur, un port, pas
/// de reverse-proxy à configurer pour un produit qui tient sur une machine.
fn with_spa(app: Router) -> Router {
    let dist = Path::new(SPA_DIR);
    if !dist.is_dir() {
        return app;
    }
    app.fallback_service(ServeDir::new(dist).fallback(ServeFile::new(dist.join("index.html"))))
}

/// Un rendu ou un appel tiers bloqué ne doit pas retenir une connexion indéfiniment.
/// (`tower::timeout` n'est pas dans les features du projet ; dix lignes suffisent.)
async fn timeout(req: Request, next: Next) -> Response {
    match tokio::time::timeout(REQUEST_TIMEOUT, next.run(req)).await {
        Ok(res) => res,
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(json!({ "error": {
                "code": "internal",
                "message": "Le traitement a pris trop de temps. Réessayez dans un instant."
            }})),
        )
            .into_response(),
    }
}

/// Hors terminal (donc en conteneur) : JSON, seul format exploitable par un agrégateur.
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,siglair=debug,tower_http=info,sqlx=warn"));
    let registry = tracing_subscriber::registry().with(filter);
    if std::io::stdout().is_terminal() {
        registry.with(tracing_subscriber::fmt::layer()).init();
    } else {
        registry
            .with(tracing_subscriber::fmt::layer().json().flatten_event(true))
            .init();
    }
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    tracing::info!("arrêt demandé");

    // L'orchestrateur attend la main en moins d'une seconde : on laisse les requêtes en vol
    // se terminer, mais une requête longue ne retient pas l'arrêt du conteneur.
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(900)).await;
        std::process::exit(0);
    });
}
