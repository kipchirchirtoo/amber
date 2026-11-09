//! The assistant is tested with a scripted stand-in for the language model, so
//! these tests prove the plumbing: the order is remembered, every number comes
//! from the engine, bad input is reported back, and nothing runs without a key.

use amber_server::assistant::{Llm, LlmReply};
use amber_server::auth::hash_password;
use amber_server::store::{NewUser, Role};
use amber_server::store::memory::MemoryStore;
use amber_server::store::{ensure_initial_rules, Store};
use amber_server::{router, AppState};
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

/// Replays prepared replies and records every message list it was shown.
struct Script {
    replies: Mutex<VecDeque<Vec<Value>>>,
    seen: Mutex<Vec<Vec<Value>>>,
}

#[async_trait]
impl Llm for Script {
    async fn complete(&self, _system: &str, _tools: &Value, messages: &[Value]) -> Result<LlmReply, String> {
        self.seen.lock().unwrap().push(messages.to_vec());
        match self.replies.lock().unwrap().pop_front() {
            Some(content) => Ok(LlmReply { content }),
            None => Err("script exhausted".into()),
        }
    }
}

fn tool(id: &str, name: &str, input: Value) -> Value {
    json!({ "type": "tool_use", "id": id, "name": name, "input": input })
}
fn text(t: &str) -> Value {
    json!({ "type": "text", "text": t })
}

struct App {
    router: axum::Router,
    token: String,
}

async fn signed_in(router: axum::Router, store: &Arc<dyn Store>) -> App {
    let password_hash = hash_password("a long password").await.unwrap();
    store.create_user(NewUser { username: "tester".into(), display_name: "Tester".into(), role: Role::Staff, password_hash }).await.unwrap();
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "username": "tester", "password": "a long password" }).to_string()))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let v: Value = serde_json::from_slice(&resp.into_body().collect().await.unwrap().to_bytes()).unwrap();
    App { router, token: v["token"].as_str().unwrap().to_string() }
}

async fn app(replies: Vec<Vec<Value>>) -> (App, Arc<Script>) {
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    ensure_initial_rules(store.as_ref()).await.unwrap();
    let script = Arc::new(Script { replies: Mutex::new(replies.into()), seen: Mutex::new(vec![]) });
    let state = AppState::new(store.clone()).with_llm(Some(script.clone()));
    (signed_in(router(state), &store).await, script)
}

async fn post(app: &App, uri: &str, body: Value) -> (StatusCode, Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", app.token))
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.router.clone().oneshot(req).await.unwrap();
    let st = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (st, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn get(app: &App, uri: &str) -> (StatusCode, Vec<u8>) {
    let req = Request::builder().uri(uri).header("authorization", format!("Bearer {}", app.token)).body(Body::empty()).unwrap();
    let resp = app.router.clone().oneshot(req).await.unwrap();
    (resp.status(), resp.into_body().collect().await.unwrap().to_bytes().to_vec())
}

const BRIEF: [(&str, u32); 3] = [("4 x 4 ft", 5), ("5 x 4 ft", 3), ("2 x 2 ft", 2)];

fn brief_windows() -> Value {
    json!(BRIEF.iter().map(|(s, q)| json!({ "size": s, "quantity": q })).collect::<Vec<_>>())
}

#[tokio::test]
async fn numbers_come_from_the_engine_and_the_order_is_remembered() {
    let (app, script) = app(vec![
        // turn 1: set the order, calculate the default view
        vec![tool("a", "set_windows", json!({ "windows": brief_windows() })), tool("b", "calculate", json!({ "view": "standard" }))],
        vec![text("Done: the aluminium table and rubber are shown.")],
        // turn 2 (a follow-up that never repeats the sizes): exact transport, then full costing
        vec![tool("c", "set_options", json!({ "transport_exact_ksh": "800" })), tool("d", "calculate", json!({ "view": "full" }))],
        vec![text("Full cost calculated.")],
    ])
    .await;

    let (st, r1) = post(&app, "/api/v1/assistant/chat", json!({ "message": "4x4 ft 5, 5x4 ft 3, 2x2 ft 2" })).await;
    assert_eq!(st, StatusCode::OK, "{r1}");
    assert_eq!(r1["reply"], "Done: the aluminium table and rubber are shown.");
    assert_eq!(r1["result"]["view"], "standard");
    // 112,585.6 mm is the hand-verified engine figure for the brief's order
    assert_eq!(r1["result"]["result"]["aluminium"]["grand_total_length_mm"], "112585.6");
    assert_eq!(r1["order"].as_array().unwrap().len(), 3);

    // what the model was shown after the calculate call is the engine's output
    {
        let seen = script.seen.lock().unwrap();
        let tool_result = seen[1].last().unwrap()["content"].as_array().unwrap().iter().find(|b| b["tool_use_id"] == "b").unwrap().clone();
        assert!(tool_result["content"].as_str().unwrap().contains("112585.6"));
        assert_eq!(tool_result["is_error"], false);
    }

    let sid = r1["session_id"].as_str().unwrap().to_string();
    let (st, r2) = post(&app, "/api/v1/assistant/chat", json!({ "session_id": sid, "message": "now the full cost, transport is 800" })).await;
    assert_eq!(st, StatusCode::OK, "{r2}");
    assert_eq!(r2["result"]["view"], "full");
    assert_eq!(r2["result"]["result"]["total_windows"], 10); // order remembered, sizes not repeated
    assert_eq!(r2["result"]["result"]["labour"]["total_ksh"], "6600");
    assert_eq!(r2["result"]["result"]["totals_consumption"]["confirmed"], true);

    // the model saw the whole earlier conversation on the follow-up
    let seen = script.seen.lock().unwrap();
    assert!(seen[2].len() >= 5);
    assert_eq!(seen[2][0]["content"], "4x4 ft 5, 5x4 ft 3, 2x2 ft 2");
}

#[tokio::test]
async fn bad_tool_input_is_reported_back_and_leaves_the_order_unchanged() {
    let (app, script) = app(vec![
        vec![tool("a", "set_windows", json!({ "windows": [{ "size": "4 x 4", "quantity": 2 }] }))], // no unit
        vec![text("Which unit did you mean for 4 x 4?")],
    ])
    .await;
    let (st, r) = post(&app, "/api/v1/assistant/chat", json!({ "message": "two windows 4 x 4" })).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["order"].as_array().unwrap().len(), 0);
    assert!(r["result"].is_null());
    let seen = script.seen.lock().unwrap();
    let res = &seen[1].last().unwrap()["content"][0];
    assert_eq!(res["is_error"], true);
    assert!(res["content"].as_str().unwrap().contains("no unit"));
}

#[tokio::test]
async fn calculating_an_empty_order_is_an_error_not_a_made_up_answer() {
    let (app, script) = app(vec![vec![tool("a", "calculate", json!({ "view": "full" }))], vec![text("I need the window sizes first.")]]).await;
    let (_, r) = post(&app, "/api/v1/assistant/chat", json!({ "message": "full cost please" })).await;
    assert!(r["result"].is_null());
    assert_eq!(script.seen.lock().unwrap()[1].last().unwrap()["content"][0]["is_error"], true);
}

#[tokio::test]
async fn existing_stock_and_quote_with_pdf() {
    let (app, _) = app(vec![
        vec![
            tool("a", "set_windows", json!({ "windows": brief_windows() })),
            tool("b", "set_options", json!({ "transport_exact_ksh": "800", "existing_stock": [{ "profile": "side_track", "length": "8 ft", "quantity": 1, "confirmed": true }] })),
            tool("c", "calculate", json!({ "view": "stock" })),
        ],
        vec![text("Shortage shown.")],
        vec![tool("d", "get_quote", json!({ "margin_percent": "20", "deposit_percent": "50", "payment_terms": "50% deposit, balance on delivery", "customer_name": "Mama Wanjiru" }))],
        vec![text("Quotation ready.")],
    ])
    .await;
    let (_, r1) = post(&app, "/api/v1/assistant/chat", json!({ "message": "I have 8 ft of side track" })).await;
    // the engine, not the model, decided how many bars are still needed
    assert_eq!(r1["result"]["view"], "stock");
    assert_eq!(r1["result"]["result"]["total_new_bars"], 20);
    let sid = r1["session_id"].as_str().unwrap();
    let (st, r2) = post(&app, "/api/v1/assistant/chat", json!({ "session_id": sid, "message": "quotation with 20% margin" })).await;
    assert_eq!(st, StatusCode::OK, "{r2}");
    let path = r2["quote_pdf_path"].as_str().unwrap();
    let (_, orders) = post_get_orders(&app).await;
    assert_eq!(orders[0]["created_by"], "tester");
    let (st, pdf) = get(&app, path).await;
    assert_eq!(st, StatusCode::OK);
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("Mama Wanjiru") && text.contains("balance on delivery"));
    assert!(!text.contains("Aluminium (")); // customer copy has no cost breakdown
}

#[tokio::test]
async fn failed_model_call_leaves_no_half_turn_and_empty_message_is_refused() {
    let (app, _) = app(vec![]).await; // script exhausted -> model error
    let (st, r) = post(&app, "/api/v1/assistant/chat", json!({ "message": "hello" })).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{r}");
    let (st, _) = post(&app, "/api/v1/assistant/chat", json!({ "message": "   " })).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _) = post(&app, "/api/v1/assistant/chat", json!({ "message": "x".repeat(4001) })).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn assistant_is_off_without_a_key() {
    let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
    ensure_initial_rules(store.as_ref()).await.unwrap();
    let app = signed_in(router(AppState::new(store.clone())), &store).await;
    let (st, r) = post(&app, "/api/v1/assistant/chat", json!({ "message": "hi" })).await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(r["error"]["code"], "assistant_disabled");
    let (st, body) = get(&app, "/api/v1/assistant/status").await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["enabled"], false);
}

async fn post_get_orders(app: &App) -> (StatusCode, Vec<Value>) {
    let (st, body) = get(app, "/api/v1/orders").await;
    (st, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn assistant_needs_a_login() {
    let (app, _) = app(vec![]).await;
    let anon = Request::builder()
        .method("POST")
        .uri("/api/v1/assistant/chat")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "message": "hi" }).to_string()))
        .unwrap();
    assert_eq!(app.router.clone().oneshot(anon).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}
