//! Runs the same API suite against real PostgreSQL, and checks the database's
//! own append-only guarantees.
//!
//! By default a temporary PostgreSQL is downloaded and started (first run
//! needs internet). To use your own server instead, set DATABASE_URL_TEST to
//! the URL of an EMPTY database.

mod common;

use amber_server::store::postgres::PgStore;
use amber_server::store::{ensure_initial_rules, Store};
use postgresql_embedded::PostgreSQL;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

async fn with_database<F, Fut>(f: F)
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    if let Ok(url) = std::env::var("DATABASE_URL_TEST") {
        f(url).await;
        return;
    }
    let mut pg = PostgreSQL::default();
    pg.setup().await.expect("cannot set up embedded PostgreSQL (needs internet on first run)");
    pg.start().await.expect("cannot start embedded PostgreSQL");
    pg.create_database("amber_test").await.expect("cannot create test database");
    let url = pg.settings().url("amber_test");
    f(url).await;
    pg.stop().await.expect("cannot stop embedded PostgreSQL");
}

#[tokio::test]
async fn api_behaviour_with_postgres() {
    with_database(|url| async move {
        let store = PgStore::connect(&url).await.unwrap();
        store.migrate().await.unwrap();
        // running migrations twice must be harmless
        store.migrate().await.unwrap();
        common::run_suite(Arc::new(store.clone())).await;

        // concurrent edits of one order never collide on a revision number
        let id = {
            let list = store.list_orders(10).await.unwrap();
            list[0].id
        };
        let rules = store.current_rules().await.unwrap();
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let s = store.clone();
            let n = rules.version_no;
            tasks.push(tokio::spawn(async move {
                s.add_revision(
                    id,
                    None,
                    amber_server::store::NewRevision { request: serde_json::json!({}), result: serde_json::json!({}), rules_version_no: n },
                )
                .await
            }));
        }
        for t in tasks {
            t.await.unwrap().expect("concurrent revision insert failed");
        }
        let d = store.get_order(id).await.unwrap();
        let mut nums: Vec<i32> = d.revisions.iter().map(|r| r.revision).collect();
        nums.sort();
        assert_eq!(nums, (1..=10).collect::<Vec<i32>>());

        // history cannot be rewritten, even by someone with direct SQL access
        let pool = PgPoolOptions::new().max_connections(1).connect(&url).await.unwrap();
        assert!(sqlx::query("UPDATE rules_versions SET note = 'tampered'").execute(&pool).await.is_err());
        assert!(sqlx::query("DELETE FROM rules_versions").execute(&pool).await.is_err());
        assert!(sqlx::query("UPDATE order_revisions SET result = '{}'::jsonb").execute(&pool).await.is_err());
        assert!(sqlx::query("DELETE FROM order_revisions").execute(&pool).await.is_err());
    })
    .await;
}

#[tokio::test]
async fn admin_changes_disabled_without_pin_postgres() {
    with_database(|url| async move {
        let store = PgStore::connect(&url).await.unwrap();
        store.migrate().await.unwrap();
        ensure_initial_rules(&store).await.unwrap();
        common::staff_cannot_change_rules(Arc::new(store)).await;
    })
    .await;
}

#[tokio::test]
async fn login_roles_and_accounts_with_postgres() {
    with_database(|url| async move {
        let store = PgStore::connect(&url).await.unwrap();
        store.migrate().await.unwrap();
        ensure_initial_rules(&store).await.unwrap();
        common::auth_suite(Arc::new(store)).await;
    })
    .await;
}

#[tokio::test]
async fn business_profile_and_letterhead_with_postgres() {
    with_database(|url| async move {
        let store = PgStore::connect(&url).await.unwrap();
        store.migrate().await.unwrap();
        ensure_initial_rules(&store).await.unwrap();
        common::business_suite(Arc::new(store)).await;
    })
    .await;
}

/// Hosted databases (Supabase) expose the public schema through a web API, so every
/// table must have row-level security on. Fails if a new table is added without it.
#[tokio::test]
async fn every_table_is_locked_down() {
    with_database(|url| async move {
        let store = PgStore::connect(&url).await.unwrap();
        store.migrate().await.unwrap();
        let pool = PgPoolOptions::new().max_connections(1).connect(&url).await.unwrap();
        let open: Vec<String> = sqlx::query_scalar(
            "SELECT c.relname::text FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace              WHERE n.nspname = current_schema() AND c.relkind = 'r' AND NOT c.relrowsecurity",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert!(open.is_empty(), "tables without row-level security: {open:?}");
    })
    .await;
}

/// Read-only look at a real database (set DATABASE_URL_TEST). Run with:
///   cargo test -p amber_server --test postgres report_database_state -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn report_database_state() {
    let url = std::env::var("DATABASE_URL_TEST").expect("set DATABASE_URL_TEST");
    let pool = sqlx::postgres::PgPoolOptions::new().max_connections(1).connect_with(std::str::FromStr::from_str(&url).unwrap()).await.unwrap();
    let tables: Vec<(String, bool)> = sqlx::query_as(
        "SELECT c.relname::text, c.relrowsecurity FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE n.nspname = current_schema() AND c.relkind = 'r' ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    println!("TABLES (name, row-level security on): {tables:?}");
    let migs: Vec<(i64, bool)> = sqlx::query_as("SELECT version, success FROM _sqlx_migrations ORDER BY version").fetch_all(&pool).await.unwrap();
    println!("MIGRATIONS: {migs:?}");
    let rules: Vec<(i32, String)> = sqlx::query_as("SELECT version_no, note FROM rules_versions ORDER BY version_no").fetch_all(&pool).await.unwrap();
    println!("RULES VERSIONS: {rules:?}");
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users").fetch_one(&pool).await.unwrap();
    println!("USERS: {users}");
    let api_grants: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM information_schema.role_table_grants WHERE table_schema = current_schema() AND grantee IN ('anon', 'authenticated')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    println!("PRIVILEGES HELD BY THE PUBLIC API ROLES (anon/authenticated) ON OUR TABLES: {api_grants}");
}
