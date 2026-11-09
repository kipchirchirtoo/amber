//! AI assistant gateway.
//!
//! The language model only INTERPRETS what the user types and calls tools. Every
//! number it can see or say comes out of the fixed calculation engine through
//! those tools; there is no tool that accepts a number to "calculate" and the
//! system prompt forbids arithmetic. The app also shows the engine's own tables
//! next to the model's words, so the figures never depend on the model's text.

use crate::api::{snapshot, AppState};
use crate::calc::{self, CalcRequest, Options, StockInput, View, WindowInput};
use crate::error::ApiError;
use crate::quote;
use amber_engine::Unit;
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

const MAX_STEPS: usize = 8;
const MAX_MESSAGE_CHARS: usize = 4000;
const MAX_HISTORY: usize = 60;
const MAX_SESSIONS: usize = 500;

// ------------------------------------------------------------------ model API

pub struct LlmReply {
    /// Anthropic content blocks: `text` and `tool_use`.
    pub content: Vec<Value>,
}

#[async_trait]
pub trait Llm: Send + Sync {
    async fn complete(&self, system: &str, tools: &Value, messages: &[Value]) -> Result<LlmReply, String>;
}

pub struct AnthropicLlm {
    client: reqwest::Client,
    key: String,
    model: String,
}

impl AnthropicLlm {
    /// Enabled only when `ANTHROPIC_API_KEY` is set. `AMBER_AI_MODEL` overrides the model.
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.trim().is_empty())?;
        let model = std::env::var("AMBER_AI_MODEL").ok().filter(|m| !m.trim().is_empty()).unwrap_or_else(|| "claude-sonnet-5-5".into());
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(90)).build().ok()?;
        Some(Self { client, key, model })
    }
}

#[async_trait]
impl Llm for AnthropicLlm {
    async fn complete(&self, system: &str, tools: &Value, messages: &[Value]) -> Result<LlmReply, String> {
        let body = json!({ "model": self.model, "max_tokens": 2048, "system": system, "tools": tools, "messages": messages });
        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("cannot reach the AI service: {}", e.without_url()))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            let detail: String = text.chars().take(300).collect();
            return Err(format!("the AI service returned {status}: {detail}"));
        }
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("unreadable AI response: {e}"))?;
        let content = v.get("content").and_then(Value::as_array).cloned().ok_or("AI response had no content")?;
        Ok(LlmReply { content })
    }
}


// ---------------------------------------------------- OpenAI-style providers
//
// Groq and Google Gemini both speak the OpenAI "chat completions" dialect (with
// tool calling). The conversation is kept in Anthropic's shape internally, so
// these functions translate it each turn. The model still only calls the same
// fixed tools; it never receives a way to calculate.

/// Anthropic-shaped history -> OpenAI chat messages (system message first).
pub fn to_openai_messages(system: &str, messages: &[Value]) -> Vec<Value> {
    let mut out = vec![json!({ "role": "system", "content": system })];
    for m in messages {
        let role = m["role"].as_str().unwrap_or("user");
        match (&m["content"], role) {
            (Value::String(text), r) => out.push(json!({ "role": r, "content": text })),
            (Value::Array(blocks), "assistant") => {
                let text: String = blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n");
                let calls: Vec<Value> = blocks
                    .iter()
                    .filter(|b| b["type"] == "tool_use")
                    .map(|b| json!({ "id": b["id"], "type": "function", "function": { "name": b["name"], "arguments": b["input"].to_string() } }))
                    .collect();
                let mut msg = json!({ "role": "assistant", "content": if text.is_empty() { Value::Null } else { Value::String(text) } });
                if !calls.is_empty() {
                    msg["tool_calls"] = Value::Array(calls);
                }
                out.push(msg);
            }
            (Value::Array(blocks), _) => {
                for b in blocks {
                    if b["type"] == "tool_result" {
                        let body = b["content"].as_str().map(String::from).unwrap_or_else(|| b["content"].to_string());
                        let body = if b["is_error"] == true { format!("ERROR: {body}") } else { body };
                        out.push(json!({ "role": "tool", "tool_call_id": b["tool_use_id"], "content": body }));
                    } else if let Some(t) = b["text"].as_str() {
                        out.push(json!({ "role": "user", "content": t }));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Anthropic tool definitions -> OpenAI function tools.
pub fn to_openai_tools(tools: &Value) -> Value {
    Value::Array(
        tools
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|t| json!({ "type": "function", "function": { "name": t["name"], "description": t["description"], "parameters": t["input_schema"] } }))
                    .collect()
            })
            .unwrap_or_default(),
    )
}

/// OpenAI chat completion -> Anthropic-shaped content blocks.
pub fn from_openai_reply(v: &Value) -> Result<LlmReply, String> {
    let msg = v.pointer("/choices/0/message").ok_or("AI response had no message")?;
    let mut content = Vec::new();
    if let Some(t) = msg["content"].as_str().filter(|t| !t.trim().is_empty()) {
        content.push(json!({ "type": "text", "text": t }));
    }
    if let Some(calls) = msg["tool_calls"].as_array() {
        for (i, c) in calls.iter().enumerate() {
            let args = c.pointer("/function/arguments").and_then(Value::as_str).unwrap_or("");
            let input: Value = if args.trim().is_empty() { json!({}) } else { serde_json::from_str(args).map_err(|e| format!("the AI sent unreadable tool arguments: {e}"))? };
            let id = c["id"].as_str().filter(|s| !s.is_empty()).map(String::from).unwrap_or_else(|| format!("call_{i}"));
            content.push(json!({ "type": "tool_use", "id": id, "name": c.pointer("/function/name").cloned().unwrap_or(Value::Null), "input": input }));
        }
    }
    if content.is_empty() {
        return Err("the AI returned an empty answer".into());
    }
    Ok(LlmReply { content })
}

/// Engine figures can carry many decimal places (for example a per-foot rate
/// worked out exactly). Before the model sees them, any decimal with more than
/// four places is rounded to four, so its replies stay readable. This is
/// display rounding only: the app shows the engine's own tables.
pub fn shorten_decimals(v: &mut Value) {
    match v {
        Value::String(s) => {
            let looks_decimal = s.split_once('.').is_some_and(|(i, f)| {
                f.len() > 4 && f.bytes().all(|b| b.is_ascii_digit()) && i.trim_start_matches('-').bytes().all(|b| b.is_ascii_digit()) && !i.trim_start_matches('-').is_empty()
            });
            if looks_decimal {
                if let Ok(d) = s.parse::<Decimal>() {
                    *s = d.round_dp_with_strategy(4, rust_decimal::RoundingStrategy::MidpointAwayFromZero).to_string();
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(shorten_decimals),
        Value::Object(o) => o.values_mut().for_each(shorten_decimals),
        _ => {}
    }
}

/// Every number found in a JSON value (numbers, and strings that are numbers).
pub fn numbers_in(v: &Value, out: &mut Vec<Decimal>) {
    match v {
        Value::String(s) => {
            if let Ok(d) = s.trim().parse::<Decimal>() {
                out.push(d);
            }
            // sizes such as "4 x 4 ft" or "1200 x 1500 mm" carry numbers too
            numbers_in_text(s, out);
        }
        Value::Number(n) => {
            if let Ok(d) = n.to_string().parse::<Decimal>() {
                out.push(d);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| numbers_in(x, out)),
        Value::Object(o) => o.values().for_each(|x| numbers_in(x, out)),
        _ => {}
    }
}

/// Numbers written in free text: digits with optional thousands commas and a decimal point.
pub fn numbers_in_text(text: &str, out: &mut Vec<Decimal>) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || ((chars[i] == ',' || chars[i] == '.') && i + 1 < chars.len() && chars[i + 1].is_ascii_digit())) {
                i += 1;
            }
            let raw: String = chars[start..i].iter().filter(|c| **c != ',').collect();
            if let Ok(d) = raw.parse::<Decimal>() {
                out.push(d);
            }
        } else {
            i += 1;
        }
    }
}

/// Figures in the assistant's reply that did not come from the engine or from
/// the user's own message. A figure counts as engine-made if it equals an
/// engine value, or an engine value shown to 0 to 4 decimal places (plain
/// display rounding). Anything else, such as a total the model added up itself,
/// is returned so the user can be warned. Small whole numbers (counts like
/// "3 windows") are not checked.
pub fn unverified_figures(reply: &str, known: &[Decimal]) -> Vec<String> {
    let mut found = Vec::new();
    numbers_in_text(reply, &mut found);
    let mut bad: Vec<String> = Vec::new();
    for v in found {
        let checked = !v.fract().is_zero() || v.abs() >= Decimal::from(100);
        if !checked {
            continue;
        }
        let ok = known.iter().any(|k| {
            k == &v || (0..=4).any(|dp| k.round_dp_with_strategy(dp, rust_decimal::RoundingStrategy::MidpointAwayFromZero) == v)
        });
        let s = v.normalize().to_string();
        if !ok && !bad.contains(&s) {
            bad.push(s);
        }
    }
    bad
}

pub struct OpenAiLlm {
    client: reqwest::Client,
    name: &'static str,
    url: String,
    key: String,
    model: String,
    /// Newer OpenAI models take `max_completion_tokens` and fix the temperature.
    modern: bool,
}

impl OpenAiLlm {
    pub fn new(name: &'static str, url: &str, key: String, model: String, modern: bool) -> Option<Self> {
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(90)).build().ok()?;
        Some(Self { client, name, url: url.to_string(), key, model, modern })
    }

    /// OpenAI. Enabled by `OPENAI_API_KEY`; `AMBER_OPENAI_MODEL` overrides the model.
    pub fn openai_from_env() -> Option<Self> {
        let key = env_nonempty("OPENAI_API_KEY")?;
        let model = env_nonempty("AMBER_OPENAI_MODEL").unwrap_or_else(|| "gpt-5.4-mini".into());
        Self::new("OpenAI", "https://api.openai.com/v1/chat/completions", key, model, true)
    }

    /// Groq. Enabled by `GROQ_API_KEY`; `AMBER_GROQ_MODEL` overrides the model.
    pub fn groq_from_env() -> Option<Self> {
        let key = env_nonempty("GROQ_API_KEY")?;
        let model = env_nonempty("AMBER_GROQ_MODEL").unwrap_or_else(|| "openai/gpt-oss-120b".into());
        Self::new("Groq", "https://api.groq.com/openai/v1/chat/completions", key, model, false)
    }

    /// Google Gemini through its OpenAI-compatible endpoint. Enabled by
    /// `GEMINI_API_KEY`; `AMBER_GEMINI_MODEL` overrides the model.
    pub fn gemini_from_env() -> Option<Self> {
        let key = env_nonempty("GEMINI_API_KEY")?;
        let model = env_nonempty("AMBER_GEMINI_MODEL").unwrap_or_else(|| "gemini-2.5-flash".into());
        Self::new("Gemini", "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions", key, model, false)
    }
}

#[async_trait]
impl Llm for OpenAiLlm {
    async fn complete(&self, system: &str, tools: &Value, messages: &[Value]) -> Result<LlmReply, String> {
        let mut body = json!({
            "model": self.model,
            "messages": to_openai_messages(system, messages),
            "tools": to_openai_tools(tools),
            "tool_choice": "auto",
        });
        if self.modern {
            body["max_completion_tokens"] = json!(4096);
        } else {
            body["max_tokens"] = json!(4096);
            body["temperature"] = json!(0);
        }
        let resp = self
            .client
            .post(&self.url)
            .bearer_auth(&self.key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("cannot reach {}: {}", self.name, e.without_url()))?;
        let status = resp.status();
        let text = resp.text().await.map_err(|e| e.to_string())?;
        if !status.is_success() {
            let detail: String = text.chars().take(300).collect();
            return Err(format!("{} returned {status}: {detail}", self.name));
        }
        let v: Value = serde_json::from_str(&text).map_err(|e| format!("unreadable response from {}: {e}", self.name))?;
        from_openai_reply(&v)
    }
}

/// Tries each configured model service in turn: if one is down, out of quota or
/// rate limited, the next one answers. Fails only if all of them fail.
pub struct FallbackLlm {
    names: Vec<String>,
    services: Vec<Arc<dyn Llm>>,
}

impl FallbackLlm {
    pub fn new(names: Vec<String>, services: Vec<Arc<dyn Llm>>) -> Self {
        Self { names, services }
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }
}

#[async_trait]
impl Llm for FallbackLlm {
    async fn complete(&self, system: &str, tools: &Value, messages: &[Value]) -> Result<LlmReply, String> {
        let mut last = String::from("no AI service is configured");
        for (name, svc) in self.names.iter().zip(&self.services) {
            match svc.complete(system, tools, messages).await {
                Ok(r) => return Ok(r),
                Err(e) => {
                    tracing::warn!("AI service {name} failed, trying the next one: {e}");
                    last = e;
                }
            }
        }
        Err(last)
    }
}

fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Builds the assistant's model service from the environment. Every service
/// that has a key is used, in this order: Anthropic, OpenAI, Gemini, Groq (the
/// first is tried first; the others are fallbacks). `AMBER_AI_PROVIDER`
/// (anthropic, openai, gemini or groq) restricts it to one. Returns None when there is no key.
pub fn llm_from_env() -> Option<FallbackLlm> {
    let only = env_nonempty("AMBER_AI_PROVIDER").map(|p| p.to_lowercase());
    let want = |n: &str| only.as_deref().is_none_or(|o| o == n);
    let mut names = Vec::new();
    let mut services: Vec<Arc<dyn Llm>> = Vec::new();
    if want("anthropic") {
        if let Some(l) = AnthropicLlm::from_env() {
            names.push(format!("Anthropic ({})", l.model));
            services.push(Arc::new(l));
        }
    }
    if want("openai") {
        if let Some(l) = OpenAiLlm::openai_from_env() {
            names.push(format!("OpenAI ({})", l.model));
            services.push(Arc::new(l));
        }
    }
    if want("gemini") {
        if let Some(l) = OpenAiLlm::gemini_from_env() {
            names.push(format!("Gemini ({})", l.model));
            services.push(Arc::new(l));
        }
    }
    if want("groq") {
        if let Some(l) = OpenAiLlm::groq_from_env() {
            names.push(format!("Groq ({})", l.model));
            services.push(Arc::new(l));
        }
    }
    if services.is_empty() {
        None
    } else {
        Some(FallbackLlm { names, services })
    }
}

// ------------------------------------------------------------------- sessions

#[derive(Default)]
pub struct Session {
    windows: Vec<WindowInput>,
    default_unit: Option<Unit>,
    options: Options,
    order_id: Option<Uuid>,
    /// The user who owns this conversation; nobody else can continue it.
    owner: Option<Uuid>,
    username: String,
    /// Order changed since it was last saved.
    dirty: bool,
    history: Vec<Value>,
}

impl Session {
    fn request(&self, view: View) -> CalcRequest {
        CalcRequest { windows: self.windows.clone(), default_unit: self.default_unit, view, options: self.options.clone() }
    }

    fn summary(&self) -> Vec<String> {
        self.windows
            .iter()
            .map(|w| {
                let size = w.size.clone().unwrap_or_else(|| format!("{} x {} mm", w.width_mm.unwrap_or_default(), w.height_mm.unwrap_or_default()));
                match &w.label {
                    Some(l) => format!("{size}: {} ({l})", w.quantity),
                    None => format!("{size}: {}", w.quantity),
                }
            })
            .collect()
    }
}

pub type Sessions = tokio::sync::Mutex<HashMap<Uuid, Arc<tokio::sync::Mutex<Session>>>>;

// ---------------------------------------------------------------- the prompt

const SYSTEM: &str = "You are the assistant inside the Amber Stores Kenya aluminium sliding-window calculator.\n\
\n\
YOUR ROLE: understand what the user types, put it into the order using the tools, ask the engine to calculate, and explain the result. You are NOT a calculator.\n\
\n\
ABSOLUTE RULES\n\
1. Never do arithmetic, never convert units yourself, never estimate, round, add up or compare numbers. Every number you state must be copied exactly from a tool result. If a figure is not in a tool result, call the right tool, or say you do not have it.\n\
2. Never invent or change a formula, price, yield or rule. The engine owns them.\n\
2a. The user already sees the engine's full tables. Do not retype tables or long lists of figures. Say what you did, then give at most a few headline figures, copied exactly as they appear in the tool result (same digits, never re-rounded, converted or recombined).\n\
3. Width always comes before height. Pass sizes to the tools exactly as the user wrote them (for example \"4 x 4 ft\"), with the unit. If the user gives a size with no unit, ask which unit; do not guess.\n\
4. Existing stock: use confirmed=true only when the user clearly says they HAVE that stock and it is usable. If they are unsure (maybe, about, I think) use confirmed=false and tell them it will not be counted. Never invent stock.\n\
5. Never invent a transport amount, kerf, deposit, margin, selling price or payment terms. If the user wants a quotation and has not given what is needed, ask. Quotations need an exact transport amount.\n\
6. Report anything the tools mark as unconfirmed or needs_fundi_confirmation, in plain words. Never turn it into a confirmed figure.\n\
7. Glass sheet counts come only from the engine. If it says to ask the fundi, say that.\n\
\n\
HOW TO WORK\n\
- The order is remembered between messages. For follow-ups (\"now the full cost\") do not ask for the order again; just call calculate with the right view.\n\
- To change the order use set_windows (replace everything), add_windows (add more), clear_order, or set_options.\n\
- Pick the view that matches the request and nothing more: standard (default: aluminium table + rubber), aluminium, rubber (actual metres only), rubber_rolls (ONLY if the user specifically asks for full-roll rubber purchasing), stock (how many 21 ft bars to buy, shortage against existing stock), panels (glass-panel sizes), fittings (rollers, locks, screws), labour, transport, glass_sheets (only if glass ordering is asked for), full (full production cost).\n\
- The app shows the engine's tables to the user automatically. Keep your reply short: say what you did, point out anything unconfirmed or missing, and quote key figures exactly as the tool gave them.\n\
- Use save_order when the user asks to save. Use get_quote for a customer quotation.\n\
- Money is Kenyan shillings (KSh). Reply in plain English, concise.";

fn tools() -> Value {
    let window = json!({
        "type": "object",
        "properties": {
            "size": { "type": "string", "description": "Width first, with unit, e.g. '4 x 4 ft', '1200 x 1500 mm', '1.2 x 1.5 m'" },
            "quantity": { "type": "integer", "minimum": 1 },
            "label": { "type": "string", "description": "Optional name such as 'toilet window'" }
        },
        "required": ["size", "quantity"]
    });
    let windows = json!({ "type": "array", "items": window, "minItems": 1 });
    json!([
        { "name": "set_windows", "description": "Replace the whole order with these window sizes and quantities.",
          "input_schema": { "type": "object", "properties": { "windows": windows.clone(), "default_unit": { "type": "string", "enum": ["mm", "ft", "m"], "description": "Only if the user says a unit applies to sizes written without one" } }, "required": ["windows"] } },
        { "name": "add_windows", "description": "Add window sizes to the existing order.",
          "input_schema": { "type": "object", "properties": { "windows": windows }, "required": ["windows"] } },
        { "name": "clear_order", "description": "Start a fresh order (also forgets options and the saved-order link).",
          "input_schema": { "type": "object", "properties": {} } },
        { "name": "set_options", "description": "Set order options. Only pass what the user stated.",
          "input_schema": { "type": "object", "properties": {
              "transport_exact_ksh": { "type": "string", "description": "Exact transport amount the user gave, in KSh" },
              "clear_transport": { "type": "boolean", "description": "Remove a previously entered exact transport amount" },
              "kerf_mm": { "type": "string", "description": "Saw kerf per cut in mm, only if the user configures it" },
              "include_glass_sheets": { "type": "boolean" },
              "existing_stock": { "type": "array", "description": "REPLACES the list of existing stock", "items": { "type": "object", "properties": {
                  "profile": { "type": "string", "enum": ["top_track", "bottom_track", "side_track", "meeting_profile", "locking_profile", "sash"] },
                  "length": { "type": "string", "description": "With unit, e.g. '8 ft'" },
                  "quantity": { "type": "integer", "minimum": 1 },
                  "confirmed": { "type": "boolean" } }, "required": ["profile", "length", "quantity", "confirmed"] } }
          } } },
        { "name": "calculate", "description": "Run the fixed calculation engine on the current order and show the result to the user.",
          "input_schema": { "type": "object", "properties": { "view": { "type": "string", "enum": ["standard", "aluminium", "rubber", "rubber_rolls", "stock", "panels", "fittings", "labour", "transport", "glass_sheets", "full"] } }, "required": ["view"] } },
        { "name": "save_order", "description": "Save the order (as a full costing) so it appears under Saved orders.",
          "input_schema": { "type": "object", "properties": { "customer_name": { "type": "string" } } } },
        { "name": "get_quote", "description": "Build a customer quotation from the saved order (saves it first if needed). Needs an exact transport amount. Only pass selling-price, margin, deposit or payment-terms values the user gave.",
          "input_schema": { "type": "object", "properties": {
              "basis": { "type": "string", "enum": ["consumption", "whole_bars", "existing_plus_purchase"] },
              "deposit_percent": { "type": "string" },
              "margin_percent": { "type": "string", "description": "Profit as a percent of the selling price" },
              "selling_price_ksh": { "type": "string" },
              "payment_terms": { "type": "string" },
              "customer_name": { "type": "string" } } } }
    ])
}

// ---------------------------------------------------------------------- tools

fn api_msg(e: ApiError) -> String {
    match e {
        ApiError::BadRequest(m) | ApiError::Unprocessable(m) => m,
        ApiError::NotFound => "not found".into(),
        _ => "internal error".into(),
    }
}

fn parse<T: for<'de> Deserialize<'de>>(input: Value) -> Result<T, String> {
    serde_json::from_value(input).map_err(|e| format!("invalid tool input: {e}"))
}

/// The big cutting-plan layouts are dropped from what the model reads; the user
/// still sees them in the app. This only shortens the model's input.
fn slim(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.remove("bins");
            m.values_mut().for_each(slim);
        }
        Value::Array(a) => a.iter_mut().for_each(slim),
        _ => {}
    }
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WindowsIn {
    windows: Vec<WindowInputIn>,
    default_unit: Option<Unit>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WindowInputIn {
    size: String,
    quantity: u32,
    label: Option<String>,
}

impl From<WindowInputIn> for WindowInput {
    fn from(w: WindowInputIn) -> Self {
        WindowInput { label: w.label, size: Some(w.size), width_mm: None, height_mm: None, quantity: w.quantity }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionsIn {
    transport_exact_ksh: Option<Decimal>,
    #[serde(default)]
    clear_transport: bool,
    kerf_mm: Option<Decimal>,
    include_glass_sheets: Option<bool>,
    existing_stock: Option<Vec<StockIn>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StockIn {
    profile: amber_engine::Profile,
    length: String,
    quantity: u32,
    confirmed: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CalcIn {
    view: View,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveIn {
    customer_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuoteIn {
    basis: Option<String>,
    deposit_percent: Option<Decimal>,
    margin_percent: Option<Decimal>,
    selling_price_ksh: Option<Decimal>,
    payment_terms: Option<String>,
    customer_name: Option<String>,
}

#[derive(Default)]
struct TurnOutput {
    result: Option<Value>,
    quote_pdf_path: Option<String>,
}

async fn save(s: &AppState, sess: &mut Session, customer: Option<String>) -> Result<Uuid, String> {
    let rev = snapshot(s, &sess.request(View::Full)).await.map_err(api_msg)?;
    let detail = match sess.order_id {
        Some(id) => s.store.add_revision(id, customer, rev).await,
        None => s.store.create_order(customer, Some(sess.username.clone()), rev).await,
    }
    .map_err(|e| api_msg(e.into()))?;
    sess.order_id = Some(detail.id);
    sess.dirty = false;
    Ok(detail.id)
}

async fn run_tool(s: &AppState, sess: &mut Session, out: &mut TurnOutput, name: &str, input: Value) -> Result<Value, String> {
    match name {
        "set_windows" | "add_windows" => {
            let i: WindowsIn = parse(input)?;
            let mut windows = if name == "add_windows" { sess.windows.clone() } else { Vec::new() };
            windows.extend(i.windows.into_iter().map(WindowInput::from));
            let unit = i.default_unit.or(if name == "add_windows" { sess.default_unit } else { None });
            let trial = CalcRequest { windows: windows.clone(), default_unit: unit, view: View::Standard, options: Options::default() };
            calc::build_order(&trial).map_err(api_msg)?; // reject bad sizes now so the model can ask the user
            sess.windows = windows;
            sess.default_unit = unit;
            sess.dirty = true;
            Ok(json!({ "order": sess.summary() }))
        }
        "clear_order" => {
            *sess = Session { history: std::mem::take(&mut sess.history), ..Session::default() };
            Ok(json!({ "order": [] }))
        }
        "set_options" => {
            let i: OptionsIn = parse(input)?;
            let mut o = sess.options.clone();
            if i.clear_transport {
                o.transport_exact_ksh = None;
            }
            if let Some(t) = i.transport_exact_ksh {
                o.transport_exact_ksh = Some(t);
            }
            if let Some(k) = i.kerf_mm {
                o.kerf_mm = k;
            }
            if let Some(g) = i.include_glass_sheets {
                o.include_glass_sheets = g;
            }
            if let Some(list) = i.existing_stock {
                o.existing_stock = list
                    .into_iter()
                    .map(|x| StockInput { profile: x.profile, length: Some(x.length), length_mm: None, quantity: x.quantity, confirmed: x.confirmed })
                    .collect();
            }
            // validate against the engine before keeping it
            let trial = CalcRequest { windows: sess.windows.clone(), default_unit: sess.default_unit, view: View::Stock, options: o.clone() };
            if !trial.windows.is_empty() {
                let rules = s.store.current_rules().await.map_err(|e| api_msg(e.into()))?;
                calc::run(&trial, &rules.rules).map_err(api_msg)?;
            }
            sess.options = o;
            sess.dirty = true;
            Ok(json!({ "ok": true, "transport_exact_ksh": sess.options.transport_exact_ksh, "existing_stock_entries": sess.options.existing_stock.len() }))
        }
        "calculate" => {
            let i: CalcIn = parse(input)?;
            if sess.windows.is_empty() {
                return Err("the order has no windows yet; ask the user for sizes and quantities".into());
            }
            let rules = s.store.current_rules().await.map_err(|e| api_msg(e.into()))?;
            let result = calc::run(&sess.request(i.view), &rules.rules).map_err(api_msg)?;
            out.result = Some(json!({ "view": i.view, "rules_version_no": rules.version_no, "result": result }));
            let mut for_model = result;
            slim(&mut for_model);
            Ok(for_model)
        }
        "save_order" => {
            let i: SaveIn = parse(input)?;
            if sess.windows.is_empty() {
                return Err("the order has no windows yet".into());
            }
            let id = save(s, sess, i.customer_name).await?;
            Ok(json!({ "saved": true, "order_id": id }))
        }
        "get_quote" => {
            let i: QuoteIn = parse(input)?;
            if sess.windows.is_empty() {
                return Err("the order has no windows yet".into());
            }
            if sess.dirty || sess.order_id.is_none() {
                save(s, sess, i.customer_name.clone()).await?;
            }
            let id = sess.order_id.expect("saved above");
            let basis = i.basis.clone().unwrap_or_else(|| "consumption".into());
            let rev = s.store.get_order(id).await.map_err(|e| api_msg(e.into()))?.latest;
            let business = s.store.get_business().await.map_err(|e| api_msg(e.into()))?;
            let terms = i.payment_terms.clone().filter(|t| !t.trim().is_empty()).or_else(|| Some(business.default_payment_terms.clone()).filter(|t| !t.trim().is_empty()));
            let pricing = quote::Pricing { selling_price_ksh: i.selling_price_ksh, margin_percent: i.margin_percent, unit_prices: None, payment_terms: terms };
            let q = quote::build(&rev.request, &rev.result, quote::Basis::parse(&basis).map_err(api_msg)?, i.deposit_percent, &pricing).map_err(api_msg)?;
            let mut path = format!("/api/v1/orders/{id}/quote.pdf?basis={basis}");
            for (k, v) in [
                ("deposit_percent", i.deposit_percent.map(|d| d.to_string())),
                ("margin_percent", i.margin_percent.map(|d| d.to_string())),
                ("selling_price_ksh", i.selling_price_ksh.map(|d| d.to_string())),
                ("payment_terms", i.payment_terms.clone()),
            ] {
                if let Some(v) = v.filter(|v| !v.trim().is_empty()) {
                    path.push_str(&format!("&{k}={}", encode(v.trim())));
                }
            }
            out.quote_pdf_path = Some(path);
            Ok(serde_json::to_value(&q).map_err(|e| e.to_string())?)
        }
        other => Err(format!("unknown tool '{other}'")),
    }
}

// ----------------------------------------------------------------- the turn

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatRequest {
    pub session_id: Option<Uuid>,
    pub message: String,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub session_id: Uuid,
    pub reply: String,
    /// The engine's own output for the last calculation in this turn, if any.
    pub result: Option<Value>,
    pub quote_pdf_path: Option<String>,
    pub order: Vec<String>,
}

/// Keeps history short without ever starting it on a tool result.
fn trim(history: &mut Vec<Value>) {
    if history.len() <= MAX_HISTORY {
        return;
    }
    let mut cut = history.len() - MAX_HISTORY;
    while cut < history.len() && !(history[cut]["role"] == "user" && history[cut]["content"].is_string()) {
        cut += 1;
    }
    history.drain(..cut);
}

pub async fn chat(s: &AppState, user: &crate::store::User, req: ChatRequest) -> Result<ChatResponse, ApiError> {
    let llm = s.llm.clone().ok_or(ApiError::AssistantDisabled)?;
    let message = req.message.trim().to_string();
    if message.is_empty() {
        return Err(ApiError::BadRequest("message is empty".into()));
    }
    if message.chars().count() > MAX_MESSAGE_CHARS {
        return Err(ApiError::BadRequest(format!("message is too long (limit {MAX_MESSAGE_CHARS} characters)")));
    }

    let id = req.session_id.unwrap_or_else(Uuid::new_v4);
    let handle = {
        let mut map = s.sessions.lock().await;
        if map.len() >= MAX_SESSIONS && !map.contains_key(&id) {
            if let Some(old) = map.keys().next().copied() {
                map.remove(&old);
            }
        }
        map.entry(id).or_default().clone()
    };
    let mut sess = handle.lock().await;
    match sess.owner {
        None => {
            sess.owner = Some(user.id);
            sess.username = user.username.clone();
        }
        Some(o) if o != user.id => return Err(ApiError::NotFound),
        Some(_) => {}
    }

    let start = sess.history.len();
    sess.history.push(json!({ "role": "user", "content": message }));
    let tools = tools();
    let mut out = TurnOutput::default();
    let mut reply = String::new();
    // every number the user typed or the engine returned during this turn
    let mut known: Vec<Decimal> = Vec::new();
    numbers_in_text(&message, &mut known);

    for step in 0..MAX_STEPS {
        let r = match llm.complete(SYSTEM, &tools, &sess.history).await {
            Ok(r) => r,
            Err(e) => {
                sess.history.truncate(start); // a failed turn leaves no half-finished history
                return Err(ApiError::Unprocessable(e));
            }
        };
        let uses: Vec<Value> = r.content.iter().filter(|b| b["type"] == "tool_use").cloned().collect();
        reply = r.content.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n");
        sess.history.push(json!({ "role": "assistant", "content": r.content }));
        if uses.is_empty() {
            break;
        }
        let mut results = Vec::new();
        for u in uses {
            let name = u["name"].as_str().unwrap_or("").to_string();
            let outcome = run_tool(s, &mut sess, &mut out, &name, u["input"].clone()).await;
            let (content, is_error) = match outcome {
                Ok(mut v) => {
                    numbers_in(&v, &mut known);
                    shorten_decimals(&mut v);
                    (v.to_string(), false)
                }
                Err(m) => (m, true),
            };
            results.push(json!({ "type": "tool_result", "tool_use_id": u["id"], "content": content, "is_error": is_error }));
        }
        sess.history.push(json!({ "role": "user", "content": results }));
        if step + 1 == MAX_STEPS {
            reply = "I could not finish that request. Please try again with a simpler instruction.".into();
        }
    }
    trim(&mut sess.history);

    // The model must never calculate. If its text contains a figure that is not
    // an engine figure (for example a total it added up itself), say so.
    let unverified = unverified_figures(&reply, &known);
    if !unverified.is_empty() {
        reply.push_str(&format!(
            "\n\nCHECK: the figure(s) {} in this text did not come from the calculation engine, so do not rely on them. Use the engine's tables below.",
            unverified.join(", ")
        ));
    }

    Ok(ChatResponse { session_id: id, reply, result: out.result, quote_pdf_path: out.quote_pdf_path, order: sess.summary() })
}
