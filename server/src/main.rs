use amber_server::store::{ensure_initial_rules, memory::MemoryStore, postgres::PgStore, Store};
use amber_server::{router, AppState};
use axum::http::HeaderValue;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// `amber_server healthcheck`: asks the running server on this machine whether it is well.
/// Used by Docker; exits 0 when healthy, 1 otherwise.
async fn healthcheck(bind: &str) -> ! {
    let port = bind.rsplit(':').next().unwrap_or("8080");
    let ok = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(3)).build() {
        Ok(c) => matches!(c.get(format!("http://127.0.0.1:{port}/health")).send().await, Ok(r) if r.status().is_success()),
        Err(_) => false,
    };
    std::process::exit(if ok { 0 } else { 1 });
}

/// Finishes in-flight requests when Docker (or Ctrl+C) asks the server to stop.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("shutting down");
}

#[tokio::main]
async fn main() {
    // Hosts such as Render give the port in PORT and expect the server on all interfaces.
    let bind = match std::env::var("PORT").ok().filter(|p| !p.trim().is_empty()) {
        Some(port) => format!("0.0.0.0:{}", port.trim()),
        None => std::env::var("BIND").unwrap_or_else(|_| "127.0.0.1:8080".into()),
    };
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        healthcheck(&bind).await;
    }
    tracing_subscriber::fmt::init();

    let store: Arc<dyn Store> = match std::env::var("DATABASE_URL") {
        Ok(url) => {
            let pg = PgStore::connect(&url).await.expect("cannot connect to PostgreSQL");
            pg.migrate().await.expect("database migration failed");
            Arc::new(pg)
        }
        Err(_) if std::env::var("AMBER_DEV_MEMORY").is_ok() => {
            tracing::warn!("AMBER_DEV_MEMORY is set: using a temporary in-memory store. NOTHING IS SAVED when the server stops.");
            Arc::new(MemoryStore::new())
        }
        Err(_) => panic!("DATABASE_URL must be set (postgres://user:pass@host/db), or set AMBER_DEV_MEMORY=1 for a throwaway demo store"),
    };
    ensure_initial_rules(store.as_ref()).await.expect("cannot seed initial rules");

    let llm: Option<Arc<dyn amber_server::assistant::Llm>> = match amber_server::assistant::llm_from_env() {
        Some(l) => {
            tracing::info!("AI assistant is on, using: {}", l.names().join(", then "));
            Some(Arc::new(l))
        }
        None => {
            tracing::warn!("no AI key set (ANTHROPIC_API_KEY, OPENAI_API_KEY, GEMINI_API_KEY or GROQ_API_KEY): the AI assistant is switched off");
            None
        }
    };
    let state = AppState::new(store).with_llm(llm);
    // First start: create the first administrator from AMBER_BOOTSTRAP_ADMIN=username:password.
    if let Some((u, p)) = std::env::var("AMBER_BOOTSTRAP_ADMIN").ok().and_then(|v| v.split_once(':').map(|(u, p)| (u.to_string(), p.to_string()))) {
        match amber_server::auth::bootstrap_admin(&state, &u, &p).await {
            Ok(true) => tracing::info!("created the first administrator '{u}'"),
            Ok(false) => {}
            Err(e) => panic!("AMBER_BOOTSTRAP_ADMIN is not usable: {e:?}"),
        }
    }
    if state.store.count_users().await.expect("cannot read users") == 0 {
        tracing::warn!("no user accounts exist: nobody can sign in. Restart once with AMBER_BOOTSTRAP_ADMIN=username:password");
    }
    let mut app = router(state).layer(TraceLayer::new_for_http());
    // Web addresses allowed to call the API from a browser, comma separated
    // (for example the Vercel address of the web app).
    if let Ok(list) = std::env::var("AMBER_CORS_ORIGIN") {
        let origins: Vec<HeaderValue> = list
            .split(',')
            .map(|o| o.trim().trim_end_matches('/'))
            .filter(|o| !o.is_empty())
            .map(|o| o.parse().unwrap_or_else(|_| panic!("AMBER_CORS_ORIGIN has an invalid origin: {o}")))
            .collect();
        if !origins.is_empty() {
            app = app.layer(CorsLayer::new().allow_origin(origins).allow_methods(Any).allow_headers(Any));
        }
    }

    let listener = tokio::net::TcpListener::bind(&bind).await.expect("cannot bind");
    tracing::info!("amber_server listening on {bind}");
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await.expect("server error");
}
