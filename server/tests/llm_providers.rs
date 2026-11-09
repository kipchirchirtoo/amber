//! The Groq / Gemini adapters and the fallback chain, tested without a network:
//! the conversation is translated to the OpenAI dialect and back, and a failing
//! model service hands over to the next one.

use amber_server::assistant::{from_openai_reply, shorten_decimals, to_openai_messages, to_openai_tools, FallbackLlm, Llm, LlmReply};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

#[test]
fn history_is_translated_with_tool_calls_and_results() {
    let history = vec![
        json!({ "role": "user", "content": "calculate 2 windows 4 x 4 ft" }),
        json!({ "role": "assistant", "content": [
            { "type": "text", "text": "Setting the order." },
            { "type": "tool_use", "id": "t1", "name": "set_windows", "input": { "windows": [{ "size": "4 x 4 ft", "quantity": 2 }] } }
        ]}),
        json!({ "role": "user", "content": [
            { "type": "tool_result", "tool_use_id": "t1", "content": "{\"ok\":true}", "is_error": false },
            { "type": "tool_result", "tool_use_id": "t2", "content": "bad unit", "is_error": true }
        ]}),
    ];
    let m = to_openai_messages("SYSTEM TEXT", &history);
    assert_eq!(m[0], json!({ "role": "system", "content": "SYSTEM TEXT" }));
    assert_eq!(m[1], json!({ "role": "user", "content": "calculate 2 windows 4 x 4 ft" }));
    assert_eq!(m[2]["role"], "assistant");
    assert_eq!(m[2]["content"], "Setting the order.");
    let call = &m[2]["tool_calls"][0];
    assert_eq!((call["id"].as_str(), call["type"].as_str(), call["function"]["name"].as_str()), (Some("t1"), Some("function"), Some("set_windows")));
    // arguments travel as a JSON string, as the OpenAI dialect requires
    let args: Value = serde_json::from_str(call["function"]["arguments"].as_str().unwrap()).unwrap();
    assert_eq!(args["windows"][0]["quantity"], 2);
    assert_eq!(m[3], json!({ "role": "tool", "tool_call_id": "t1", "content": "{\"ok\":true}" }));
    assert_eq!(m[4]["role"], "tool");
    assert_eq!(m[4]["content"], "ERROR: bad unit");
}

#[test]
fn a_tool_only_assistant_turn_has_null_content() {
    let history = vec![json!({ "role": "assistant", "content": [{ "type": "tool_use", "id": "a", "name": "calculate", "input": { "view": "rubber" } }] })];
    let m = to_openai_messages("s", &history);
    assert!(m[1]["content"].is_null());
    assert_eq!(m[1]["tool_calls"].as_array().unwrap().len(), 1);
}

#[test]
fn tools_become_openai_functions() {
    let tools = json!([{ "name": "calculate", "description": "Run it", "input_schema": { "type": "object", "properties": { "view": { "type": "string" } } } }]);
    let t = to_openai_tools(&tools);
    assert_eq!(t[0]["type"], "function");
    assert_eq!(t[0]["function"]["name"], "calculate");
    assert_eq!(t[0]["function"]["parameters"]["properties"]["view"]["type"], "string");
}

#[test]
fn replies_become_text_and_tool_use_blocks() {
    let reply = json!({ "choices": [{ "message": {
        "content": "Working on it.",
        "tool_calls": [
            { "id": "", "type": "function", "function": { "name": "calculate", "arguments": "{\"view\":\"rubber\"}" } },
            { "id": "xyz", "type": "function", "function": { "name": "clear_order", "arguments": "" } }
        ]
    }}]});
    let r = from_openai_reply(&reply).unwrap();
    assert_eq!(r.content[0], json!({ "type": "text", "text": "Working on it." }));
    assert_eq!(r.content[1]["type"], "tool_use");
    assert_eq!(r.content[1]["id"], "call_0", "a missing id is replaced so results can be matched");
    assert_eq!(r.content[1]["input"]["view"], "rubber");
    assert_eq!(r.content[2]["id"], "xyz");
    assert_eq!(r.content[2]["input"], json!({}));

    let plain = from_openai_reply(&json!({ "choices": [{ "message": { "content": "Hello" } }] })).unwrap();
    assert_eq!(plain.content.len(), 1);
    assert!(from_openai_reply(&json!({ "choices": [{ "message": { "content": "  " } }] })).is_err());
    assert!(from_openai_reply(&json!({ "nope": 1 })).is_err());
    let bad = json!({ "choices": [{ "message": { "tool_calls": [{ "id": "1", "function": { "name": "x", "arguments": "{oops" } }] } }] });
    assert!(from_openai_reply(&bad).is_err(), "unreadable tool arguments are an error, never guessed");
}

#[test]
fn long_decimals_are_shortened_for_the_model_only() {
    let mut v = json!({
        "a": "64311.961004874390701162354707",
        "b": "1199.20",
        "c": "12345",
        "d": ["675.27559055118110236220472441", { "e": "-0.123456789" }],
        "id": "123e4567-e89b-12d3-a456-426614174000",
        "n": 3
    });
    shorten_decimals(&mut v);
    assert_eq!(v["a"], "64311.9610");
    assert_eq!(v["b"], "1199.20");
    assert_eq!(v["c"], "12345");
    assert_eq!(v["d"][0], "675.2756");
    assert_eq!(v["d"][1]["e"], "-0.1235");
    assert_eq!(v["id"], "123e4567-e89b-12d3-a456-426614174000");
    assert_eq!(v["n"], 3);
}

struct Stub(Result<Vec<Value>, String>);

#[async_trait]
impl Llm for Stub {
    async fn complete(&self, _s: &str, _t: &Value, _m: &[Value]) -> Result<LlmReply, String> {
        self.0.clone().map(|content| LlmReply { content })
    }
}

#[tokio::test]
async fn a_failing_service_hands_over_to_the_next() {
    let chain = FallbackLlm::new(
        vec!["first".into(), "second".into()],
        vec![Arc::new(Stub(Err("429 quota".into()))), Arc::new(Stub(Ok(vec![json!({ "type": "text", "text": "from second" })])))],
    );
    let r = chain.complete("s", &json!([]), &[]).await.unwrap();
    assert_eq!(r.content[0]["text"], "from second");

    let all_down = FallbackLlm::new(vec!["a".into(), "b".into()], vec![Arc::new(Stub(Err("down A".into()))), Arc::new(Stub(Err("down B".into())))]);
    assert_eq!(all_down.complete("s", &json!([]), &[]).await.err().unwrap(), "down B");
}

#[test]
fn a_total_the_model_added_up_itself_is_flagged() {
    use amber_server::assistant::{numbers_in, numbers_in_text, unverified_figures};
    let mut known = Vec::new();
    numbers_in(&json!({ "aluminium_ksh": "64311.9610", "rubber_ksh": "3622.0646", "fittings_ksh": "6264.00", "labour_ksh": "12000", "total_low_ksh": "86898.0256" }), &mut known);
    numbers_in_text("Please cost 10 windows of 4 x 4 ft", &mut known);

    // copied figures, and plain display rounding, pass
    assert!(unverified_figures("Aluminium KSh 64311.9610, rubber KSh 3,622.06, labour KSh 12,000 for 10 windows.", &known).is_empty());
    assert!(unverified_figures("Total between KSh 86,898.0256 and ...", &known).is_empty());
    // a sum the model worked out is caught (64311.9610 + 3622.0646 + 6264 + 12000)
    assert_eq!(unverified_figures("Total (excluding transport): KSh 86198.0256", &known), vec!["86198.0256".to_string()]);
    // small counts are not checked; an invented large figure is
    assert!(unverified_figures("That is 3 windows in 2 sizes.", &known).is_empty());
    assert_eq!(unverified_figures("You need 450 bars", &known), vec!["450".to_string()]);
}
