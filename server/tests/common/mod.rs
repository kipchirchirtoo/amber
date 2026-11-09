//! One behavioural suite, run against every Store implementation.

use amber_server::auth::hash_password;
use amber_server::store::{ensure_initial_rules, NewUser, Role, Store};
use amber_server::{router, AppState};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

pub const PIN: &str = "1234";

pub const PASSWORD: &str = "correct horse 42";

pub async fn make_user(store: &Arc<dyn Store>, username: &str, role: Role) {
    let password_hash = hash_password(PASSWORD).await.unwrap();
    store.create_user(NewUser { username: username.into(), display_name: username.to_uppercase(), role, password_hash }).await.unwrap();
}

/// A running API with one administrator ("owner") and one staff user ("fundi") already signed in.
pub struct Api {
    router: axum::Router,
    admin: String,
    staff: String,
}

impl Api {
    /// `_pin` is kept so call sites read the same: `Some(PIN)` in a call means "as the administrator".
    pub async fn new(store: Arc<dyn Store>, _pin: Option<&str>) -> Api {
        ensure_initial_rules(store.as_ref()).await.unwrap();
        make_user(&store, "owner", Role::Admin).await;
        make_user(&store, "fundi", Role::Staff).await;
        let router = router(AppState::new(store));
        let mut api = Api { router, admin: String::new(), staff: String::new() };
        api.admin = api.login("owner", PASSWORD).await;
        api.staff = api.login("fundi", PASSWORD).await;
        api
    }

    pub async fn login(&self, user: &str, password: &str) -> String {
        let (st, v) = self.raw("POST", "/api/v1/auth/login", None, Some(json!({ "username": user, "password": password }))).await;
        assert_eq!(st, StatusCode::OK, "login {user}: {v}");
        v["token"].as_str().unwrap().to_string()
    }

    /// Any request with an explicit login token (or none).
    pub async fn raw(&self, method: &str, uri: &str, token: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(uri).header("content-type", "application/json");
        if let Some(t) = token {
            req = req.header("authorization", format!("Bearer {t}"));
        }
        let req = req.body(body.map_or_else(Body::empty, |b| Body::from(b.to_string()))).unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    /// PUT a raw (non-JSON) body such as a picture.
    pub async fn put_bytes(&self, uri: &str, token: &str, body: Vec<u8>) -> StatusCode {
        let req = Request::builder().method("PUT").uri(uri).header("authorization", format!("Bearer {token}")).body(Body::from(body)).unwrap();
        self.router.clone().oneshot(req).await.unwrap().status()
    }

    pub async fn raw_bytes(&self, uri: &str, token: Option<&str>) -> (StatusCode, Vec<u8>) {
        let mut req = Request::builder().method("GET").uri(uri);
        if let Some(t) = token {
            req = req.header("authorization", format!("Bearer {t}"));
        }
        let resp = self.router.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
        (resp.status(), resp.into_body().collect().await.unwrap().to_bytes().to_vec())
    }

    /// `pin`: None = signed in as staff, Some(PIN) = signed in as the administrator,
    /// anything else = not signed in.
    pub async fn call(&self, method: &str, uri: &str, pin: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
        let token = match pin {
            None => Some(self.staff.as_str()),
            Some(PIN) => Some(self.admin.as_str()),
            Some(_) => None,
        };
        self.raw(method, uri, token, body).await
    }
}

impl Api {
    /// Raw GET for non-JSON responses (as staff): (status, content-type, body).
    pub async fn get_bytes(&self, uri: &str) -> (StatusCode, String, Vec<u8>) {
        let req = Request::builder().method("GET").uri(uri).header("authorization", format!("Bearer {}", self.staff)).body(Body::empty()).unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
        (status, ct, resp.into_body().collect().await.unwrap().to_bytes().to_vec())
    }
}

fn brief_windows() -> Value {
    json!([
        { "size": "4 x 4 ft", "quantity": 5 },
        { "size": "5 x 4 ft", "quantity": 3 },
        { "size": "2 x 2 ft", "label": "toilet window", "quantity": 2 }
    ])
}

const ZERO_ID: &str = "00000000-0000-0000-0000-000000000000";

pub async fn run_suite(store: Arc<dyn Store>) {
    let api = Api::new(store, Some(PIN)).await;

    // --- default output is aluminium + rubber only ---
    let (st, v) = api.call("POST", "/api/v1/calculate", None, Some(json!({ "windows": brief_windows() }))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["rules_version_no"], 1);
    assert_eq!(v["result"]["aluminium"]["grand_total_length_mm"], "112585.6");
    assert_eq!(v["result"]["aluminium"]["grand_total_pieces"], 120);
    assert_eq!(v["result"]["rubber"]["total_metres"], "60.2656");
    assert_eq!(v["result"].as_object().unwrap().len(), 2);

    // --- width/height in mm, and other views ---
    let mm = json!({ "windows": [{ "width_mm": "1219.2", "height_mm": "1219.2", "quantity": 1 }], "view": "aluminium" });
    let (st, v) = api.call("POST", "/api/v1/calculate", None, Some(mm)).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["result"]["grand_total_length_mm"], "12112.0");

    // 5 windows of 4x4 ft: 1 + 1 + 2 + 2 + 2 + 2 bars (5 cuts fit per bar for most profiles, 10 for sash)
    let stock = json!({ "windows": [{ "size": "4 x 4 ft", "quantity": 5 }], "view": "stock" });
    let (_, v) = api.call("POST", "/api/v1/calculate", None, Some(stock)).await;
    assert_eq!(v["result"]["status"], "verified");
    assert_eq!(v["result"]["total_new_bars"], 10);

    // existing stock typed with a unit is parsed by the engine; only confirmed stock is used
    let with_stock = |confirmed: bool| {
        json!({ "windows": [{ "size": "4 x 4 ft", "quantity": 1 }], "view": "stock",
                "options": { "existing_stock": [{ "profile": "side_track", "length": "8 ft", "quantity": 1, "confirmed": confirmed }] } })
    };
    let (st, v) = api.call("POST", "/api/v1/calculate", None, Some(with_stock(true))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["result"]["profiles"][2]["existing_pieces_used"], 1);
    assert_eq!(v["result"]["total_new_bars"], 5);
    let (_, v) = api.call("POST", "/api/v1/calculate", None, Some(with_stock(false))).await;
    assert_eq!(v["result"]["profiles"][2]["existing_pieces_used"], 0);
    assert_eq!(v["result"]["total_new_bars"], 6);
    let bare = json!({ "windows": [{ "size": "4 x 4 ft", "quantity": 1 }], "view": "stock",
                       "options": { "existing_stock": [{ "profile": "side_track", "length": "8", "quantity": 1, "confirmed": true }] } });
    let (st, _) = api.call("POST", "/api/v1/calculate", None, Some(bare)).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "a length with no unit must be refused");

    let tr = json!({ "windows": brief_windows(), "view": "transport" });
    let (_, v) = api.call("POST", "/api/v1/calculate", None, Some(tr)).await;
    assert_eq!(v["result"]["status"], "unconfirmed");
    assert_eq!(v["result"]["min_ksh"], "700");

    // whole-roll rubber only when specifically requested; the default view never shows it
    let rolls = json!({ "windows": brief_windows(), "view": "rubber_rolls" });
    let (st, v) = api.call("POST", "/api/v1/calculate", None, Some(rolls)).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["result"]["consumption"]["total_metres"], "60.2656");
    assert_eq!(v["result"]["rolls"], 2);
    assert_eq!(v["result"]["full_roll_cost_ksh"], "2500");
    assert_eq!(v["result"]["unused_metres"], "15.7344");

    let lab = json!({ "windows": brief_windows(), "view": "labour" });
    let (_, v) = api.call("POST", "/api/v1/calculate", None, Some(lab)).await;
    assert_eq!(v["result"]["total_ksh"], "6600");

    // --- bad input is rejected clearly, never guessed ---
    let (st, v) = api.call("POST", "/api/v1/calculate", None, Some(json!({ "windows": [{ "size": "4 x 5", "quantity": 1 }] }))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "no unit must be refused: {v}");
    let (st, _) = api.call("POST", "/api/v1/calculate", None, Some(json!({ "windows": [{ "quantity": 1 }] }))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _) = api
        .call("POST", "/api/v1/calculate", None, Some(json!({ "windows": [{ "size": "4 x 4 ft", "width_mm": "1", "height_mm": "1", "quantity": 1 }] })))
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, v) = api
        .call("POST", "/api/v1/calculate", None, Some(json!({ "windows": [{ "size": "4 x 4 ft", "quantity": 1 }], "kerf": 3 })))
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "a misspelt option must not be silently ignored: {v}");
    assert_eq!(v["error"]["code"], "bad_request");
    let (st, _) = api.call("POST", "/api/v1/calculate", None, Some(json!({ "windows": [{ "size": "4 x 4 ft", "quantity": 0 }] }))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    let (st, _) = api.call("POST", "/api/v1/calculate", None, Some(json!({ "windows": [{ "size": "20 x 500 mm", "quantity": 1 }] }))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // --- orders: save, list, retrieve ---
    let full = json!({ "windows": brief_windows(), "view": "full", "options": { "transport_exact_ksh": "800" } });
    let (st, o) = api.call("POST", "/api/v1/orders", None, Some(json!({ "customer_name": "Mama Wanjiru", "request": full }))).await;
    assert_eq!(st, StatusCode::CREATED, "{o}");
    let id = o["id"].as_str().unwrap().to_string();
    assert_eq!(o["latest"]["revision"], 1);
    assert_eq!(o["latest"]["rules_version_no"], 1);
    let rev1_result = o["latest"]["result"].clone();
    assert_eq!(rev1_result["totals_whole_bars"]["aluminium_ksh"], "40000"); // 20 bars x 2000

    let (st, list) = api.call("GET", "/api/v1/orders", None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["customer_name"], "Mama Wanjiru");
    assert_eq!(list[0]["revision_count"], 1);
    let (st, _) = api.call("GET", &format!("/api/v1/orders/{ZERO_ID}"), None, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = api.call("PUT", &format!("/api/v1/orders/{ZERO_ID}"), None, Some(json!({ "request": full }))).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // --- quotations come from the saved snapshot ---
    let (st, q) = api.call("GET", &format!("/api/v1/orders/{id}/quote"), None, None).await;
    assert_eq!(st, StatusCode::OK, "{q}");
    assert_eq!(q["total_low_ksh"], "48035.31"); // consumption basis, transport 800 confirmed
    assert_eq!(q["total_high_ksh"], "48035.31");
    assert_eq!(q["confirmed"], true);
    assert!(q["deposit_ksh"].is_null());
    let (_, q) = api.call("GET", &format!("/api/v1/orders/{id}/quote?basis=whole_bars&deposit_percent=50"), None, None).await;
    assert_eq!(q["total_low_ksh"], "52856.71");
    assert_eq!(q["deposit_ksh"], "26428.36");
    assert_eq!(q["balance_ksh"], "26428.35"); // deposit + balance always equals the total
    let (st, _) = api.call("GET", &format!("/api/v1/orders/{id}/quote?basis=nonsense"), None, None).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _) = api.call("GET", &format!("/api/v1/orders/{id}/quote?deposit_percent=0"), None, None).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _) = api.call("GET", &format!("/api/v1/orders/{id}/quote?basis=existing_plus_purchase"), None, None).await;
    assert_eq!(st, StatusCode::BAD_REQUEST); // no existing stock in this order
    let (st, ct, pdf) = api.get_bytes(&format!("/api/v1/orders/{id}/quote.pdf?deposit_percent=50")).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(ct, "application/pdf");
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.starts_with("%PDF-1.4") && text.contains("Mama Wanjiru") && text.contains("48,035.31"));
    // selling price: only when the owner supplies a margin or an exact price
    let base = format!("/api/v1/orders/{id}/quote");
    let (_, q) = api.call("GET", &format!("{base}?margin_percent=20&deposit_percent=50"), None, None).await;
    assert_eq!(q["selling"]["cost_ksh"], "48035.31");
    assert_eq!(q["selling"]["selling_price_ksh"], "60044.14");
    assert_eq!(q["selling"]["profit_ksh"], "12008.83");
    assert_eq!(q["selling"]["margin_percent"], "20.00");
    assert_eq!(q["deposit_ksh"], "30022.07"); // deposit is taken on the selling price
    assert_eq!(q["balance_ksh"], "30022.07");
    let (_, q) = api.call("GET", &format!("{base}?selling_price_ksh=55000"), None, None).await;
    assert_eq!(q["selling"]["profit_ksh"], "6964.69");
    assert_eq!(q["selling"]["margin_percent"], "12.66");
    assert!(q["payment_terms"].is_null());
    // unit selling prices per window line: the server only multiplies and adds what the owner typed
    let (st, q) = api.call("GET", &format!("{base}?unit_prices=5000,6000,3000&deposit_percent=50"), None, None).await;
    assert_eq!(st, StatusCode::OK, "{q}");
    let num = |v: &serde_json::Value| v.as_str().unwrap().parse::<rust_decimal::Decimal>().unwrap();
    let d = |s: &str| s.parse::<rust_decimal::Decimal>().unwrap();
    let pw = q["priced_windows"].as_array().unwrap();
    assert_eq!(pw.len(), 3);
    assert_eq!((num(&pw[0]["line_total_ksh"]), pw[0]["quantity"].as_u64()), (d("25000"), Some(5))); // 5 x 5,000
    assert_eq!(num(&pw[1]["line_total_ksh"]), d("18000")); // 3 x 6,000
    assert_eq!(num(&pw[2]["line_total_ksh"]), d("6000")); // 2 x 3,000
    assert_eq!(num(&q["selling"]["selling_price_ksh"]), d("49000"));
    assert_eq!(num(&q["selling"]["profit_ksh"]), d("964.69")); // 49,000 - cost 48,035.31
    assert_eq!(num(&q["selling"]["margin_percent"]), d("1.97"));
    assert_eq!((num(&q["deposit_ksh"]), num(&q["balance_ksh"])), (d("24500"), d("24500")));
    for bad in [
        "unit_prices=5000,6000",                         // one price per window line, no more, no fewer
        "unit_prices=5000,6000,3000,1",
        "unit_prices=5000,6000,x",
        "unit_prices=5000,6000,0",
        "unit_prices=5000,6000,3000&margin_percent=10", // only one pricing method at a time
        "unit_prices=5000,6000,3000&selling_price_ksh=60000",
    ] {
        let (st, _) = api.call("GET", &format!("{base}?{bad}"), None, None).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (_, _, pdf) = api.get_bytes(&format!("{base}.pdf?unit_prices=5000,6000,3000")).await;
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("25,000.00") && text.contains("18,000.00") && text.contains("49,000.00"));
    assert!(!text.contains("48,035.31") && !text.contains("964.69"), "customer copy must not reveal cost or profit");
    for bad in ["margin_percent=100", "margin_percent=-1", "selling_price_ksh=0", "margin_percent=10&selling_price_ksh=60000"] {
        let (st, _) = api.call("GET", &format!("{base}?{bad}"), None, None).await;
        assert_eq!(st, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (_, ctext, pdf) = api.get_bytes(&format!("{base}.pdf?margin_percent=20&payment_terms=50%25%20deposit%2C%20balance%20on%20delivery")).await;
    assert_eq!(ctext, "application/pdf");
    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("60,044.14") && text.contains("balance on delivery"));
    assert!(!text.contains("35,178.60") && !text.contains("12,008.83"), "customer copy must not reveal cost or profit");
    // transport not confirmed -> a range, flagged, and no deposit/balance
    let unconf = json!({ "windows": brief_windows(), "view": "full" });
    let (_, u) = api.call("POST", "/api/v1/orders", None, Some(json!({ "request": unconf }))).await;
    let uid = u["id"].as_str().unwrap();
    let (_, q) = api.call("GET", &format!("/api/v1/orders/{uid}/quote?deposit_percent=50"), None, None).await;
    let (st, _) = api.call("GET", &format!("/api/v1/orders/{uid}/quote?margin_percent=20"), None, None).await;
    assert_eq!(st, StatusCode::BAD_REQUEST); // no selling price on an unconfirmed cost
    assert_eq!(q["confirmed"], false);
    assert_eq!(q["total_low_ksh"], "47935.31");
    assert_eq!(q["total_high_ksh"], "48235.31");
    assert!(q["deposit_ksh"].is_null());
    // a calculation without full costing cannot be quoted
    let basic = json!({ "windows": brief_windows(), "view": "standard" });
    let (_, b) = api.call("POST", "/api/v1/orders", None, Some(json!({ "request": basic }))).await;
    let (st, _) = api.call("GET", &format!("/api/v1/orders/{}/quote", b["id"].as_str().unwrap()), None, None).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // --- stock and offcuts ---
    let (st, items) = api.call("GET", "/api/v1/stock", None, None).await;
    assert_eq!((st, items.as_array().unwrap().len()), (StatusCode::OK, 0));
    let (st, it) = api.call("POST", "/api/v1/stock", None, Some(json!({ "profile": "side_track", "length": "8 ft", "quantity": 2 }))).await;
    assert_eq!(st, StatusCode::CREATED, "{it}");
    assert_eq!(it["length_mm"], "2438.4");
    assert_eq!(it["confirmed"], false); // never assumed available
    let sid = it["id"].as_str().unwrap().to_string();
    let (st, it) = api
        .call("PUT", &format!("/api/v1/stock/{sid}"), None, Some(json!({ "profile": "sash", "length_mm": "1500", "quantity": 1, "confirmed": true, "note": "  offcut  " })))
        .await;
    assert_eq!(st, StatusCode::OK, "{it}");
    assert_eq!((it["profile"].as_str(), it["confirmed"].as_bool(), it["note"].as_str()), (Some("sash"), Some(true), Some("offcut")));
    for bad in [
        json!({ "profile": "sash", "length": "8", "quantity": 1 }),
        json!({ "profile": "sash", "length": "0 ft", "quantity": 1 }),
        json!({ "profile": "sash", "length": "8 ft", "quantity": 0 }),
        json!({ "profile": "sash", "length": "8 ft", "length_mm": "5", "quantity": 1 }),
        json!({ "profile": "bogus", "length": "8 ft", "quantity": 1 }),
    ] {
        let (st, _) = api.call("POST", "/api/v1/stock", None, Some(bad.clone())).await;
        assert!(st == StatusCode::BAD_REQUEST || st == StatusCode::UNPROCESSABLE_ENTITY, "{bad} -> {st}");
    }
    let (st, _) = api.call("PUT", &format!("/api/v1/stock/{ZERO_ID}"), None, Some(json!({ "profile": "sash", "length": "1 ft", "quantity": 1 }))).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = api.call("DELETE", &format!("/api/v1/stock/{sid}"), None, None).await;
    assert_eq!(st, StatusCode::NO_CONTENT);
    let (st, _) = api.call("DELETE", &format!("/api/v1/stock/{sid}"), None, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (_, items) = api.call("GET", "/api/v1/stock", None, None).await;
    assert_eq!(items.as_array().unwrap().len(), 0);

    // --- rule changes need an administrator ---
    let (_, cur) = api.call("GET", "/api/v1/settings/rules", None, None).await;
    assert_eq!(cur["version_no"], 1);
    let mut priced = cur["rules"].clone();
    priced["stock_price_ksh"] = json!("2200");
    let change = json!({ "rules": priced, "note": "Supplier raised price" });
    let (st, _) = api.call("PUT", "/api/v1/settings/rules", None, Some(change.clone())).await;
    assert_eq!(st, StatusCode::FORBIDDEN); // staff may not change prices
    let (st, _) = api.call("PUT", "/api/v1/settings/rules", Some("9999"), Some(change.clone())).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = api.call("PUT", "/api/v1/settings/rules", Some(PIN), Some(json!({ "rules": priced, "note": "  " }))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, v) = api.call("PUT", "/api/v1/settings/rules", Some(PIN), Some(change)).await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    assert_eq!(v["version_no"], 2);
    assert_eq!(v["authorised_formula_change"], false);
    assert_eq!(v["rules"]["version"], "amber-v2");

    // identical rules are refused
    let (_, cur2) = api.call("GET", "/api/v1/settings/rules", None, None).await;
    let (st, _) = api.call("PUT", "/api/v1/settings/rules", Some(PIN), Some(json!({ "rules": cur2["rules"], "note": "again" }))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // --- history is preserved: the saved order did not change ---
    let (_, o) = api.call("GET", &format!("/api/v1/orders/{id}"), None, None).await;
    assert_eq!(o["latest"]["result"], rev1_result);
    assert_eq!(o["latest"]["rules_version_no"], 1);

    // new calculations use the new price: 20 bars x 2200
    let (_, v) = api.call("POST", "/api/v1/calculate", None, Some(full.clone())).await;
    assert_eq!(v["rules_version_no"], 2);
    assert_eq!(v["result"]["totals_whole_bars"]["aluminium_ksh"], "44000");

    // editing the order adds revision 2 with the new rules; revision 1 stays intact
    let (st, o) = api.call("PUT", &format!("/api/v1/orders/{id}"), None, Some(json!({ "request": full }))).await;
    assert_eq!(st, StatusCode::OK, "{o}");
    assert_eq!(o["latest"]["revision"], 2);
    assert_eq!(o["latest"]["rules_version_no"], 2);
    assert_eq!(o["customer_name"], "Mama Wanjiru");
    assert_eq!(o["revisions"].as_array().unwrap().len(), 2);
    let (st, r1) = api.call("GET", &format!("/api/v1/orders/{id}/revisions/1"), None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(r1["result"], rev1_result);
    let (st, _) = api.call("GET", &format!("/api/v1/orders/{id}/revisions/9"), None, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // --- formulas are protected ---
    let mut formula = cur2["rules"].clone();
    formula["cuts"][0]["adjust_mm"] = json!("-25");
    let (st, v) = api.call("PUT", "/api/v1/settings/rules", Some(PIN), Some(json!({ "rules": formula, "note": "try" }))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(v["error"]["message"].as_str().unwrap().contains("authorise_formula_change"));
    let (st, v) = api
        .call("PUT", "/api/v1/settings/rules", Some(PIN), Some(json!({ "rules": formula, "note": "owner approved", "authorise_formula_change": true })))
        .await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    assert_eq!(v["authorised_formula_change"], true);
    assert_eq!(v["version_no"], 3);

    // an unusable rule set is rejected by the engine's validation
    let mut broken = cur2["rules"].clone();
    broken["cuts"].as_array_mut().unwrap().pop();
    let (st, _) = api
        .call("PUT", "/api/v1/settings/rules", Some(PIN), Some(json!({ "rules": broken, "note": "x", "authorise_formula_change": true })))
        .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);

    // --- full price/setting history is kept, newest first ---
    let (_, h) = api.call("GET", "/api/v1/settings/rules/history", None, None).await;
    let h = h.as_array().unwrap();
    assert_eq!(h.len(), 3);
    assert_eq!(h[0]["version_no"], 3);
    assert_eq!(h[1]["rules"]["stock_price_ksh"], "2200");
    assert_eq!(h[2]["rules"]["stock_price_ksh"], "2000");

    let (st, _) = api.call("GET", "/health", None, None).await;
    assert_eq!(st, StatusCode::OK);
}

/// Staff can calculate and save but never change prices; neither can a stranger.
pub async fn staff_cannot_change_rules(store: Arc<dyn Store>) {
    let api = Api::new(store, None).await;
    let (_, cur) = api.call("GET", "/api/v1/settings/rules", None, None).await;
    let body = json!({ "rules": cur["rules"], "note": "x" });
    let (st, _) = api.call("PUT", "/api/v1/settings/rules", None, Some(body.clone())).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, _) = api.call("PUT", "/api/v1/settings/rules", Some("nobody"), Some(body)).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
}

/// Login, roles, lock-out, deactivation, password change, tickets.
pub async fn auth_suite(store: Arc<dyn Store>) {
    let api = Api::new(store, None).await;
    let owner = api.login("owner", PASSWORD).await;
    let fundi = api.login("fundi", PASSWORD).await;

    // --- nothing works without a login, except health and the login itself ---
    for (m, uri) in [("GET", "/api/v1/orders"), ("GET", "/api/v1/stock"), ("GET", "/api/v1/settings/rules"), ("GET", "/api/v1/auth/me"), ("GET", "/api/v1/users")] {
        let (st, v) = api.raw(m, uri, None, None).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED, "{uri}: {v}");
    }
    let (st, _) = api.raw("GET", "/api/v1/orders", Some("not-a-real-token"), None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = api.raw("GET", "/health", None, None).await;
    assert_eq!(st, StatusCode::OK);

    // --- wrong credentials look identical whichever part is wrong ---
    let (st1, v1) = api.raw("POST", "/api/v1/auth/login", None, Some(json!({ "username": "owner", "password": "wrong password" }))).await;
    let (st2, v2) = api.raw("POST", "/api/v1/auth/login", None, Some(json!({ "username": "nosuchuser", "password": "wrong password" }))).await;
    assert_eq!((st1, st2), (StatusCode::UNAUTHORIZED, StatusCode::UNAUTHORIZED));
    assert_eq!(v1, v2);
    let (_, me) = api.raw("GET", "/api/v1/auth/me", Some(&owner), None).await;
    assert_eq!((me["username"].as_str(), me["role"].as_str()), (Some("owner"), Some("admin")));
    let (_, me) = api.raw("GET", "/api/v1/auth/me", Some(&fundi), None).await;
    assert_eq!(me["role"], "staff");
    assert!(me.get("password_hash").is_none());
    // usernames are not case sensitive
    api.login("OWNER", PASSWORD).await;

    // --- only administrators manage users ---
    let (st, _) = api.raw("GET", "/api/v1/users", Some(&fundi), None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let new_user = json!({ "username": "amina", "display_name": "Amina W", "role": "staff", "password": "short" });
    let (st, _) = api.raw("POST", "/api/v1/users", Some(&fundi), Some(new_user.clone())).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    let (st, v) = api.raw("POST", "/api/v1/users", Some(&owner), Some(new_user.clone())).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{v}"); // password too short
    let mut good = new_user.clone();
    good["password"] = json!("long enough pw");
    let (st, amina) = api.raw("POST", "/api/v1/users", Some(&owner), Some(good.clone())).await;
    assert_eq!(st, StatusCode::CREATED, "{amina}");
    assert!(amina.get("password_hash").is_none());
    let amina_id = amina["id"].as_str().unwrap().to_string();
    good["username"] = json!("AMINA");
    let (st, _) = api.raw("POST", "/api/v1/users", Some(&owner), Some(good)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST); // duplicate, case-insensitive
    let mut bad_name = new_user.clone();
    bad_name["username"] = json!("a b");
    bad_name["password"] = json!("long enough pw");
    let (st, _) = api.raw("POST", "/api/v1/users", Some(&owner), Some(bad_name)).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // --- who did what ---
    let amina_tok = {
        let (_, v) = api.raw("POST", "/api/v1/auth/login", None, Some(json!({ "username": "amina", "password": "long enough pw" }))).await;
        v["token"].as_str().unwrap().to_string()
    };
    let req = json!({ "windows": [{ "size": "4 x 4 ft", "quantity": 2 }], "view": "full", "options": { "transport_exact_ksh": "500" } });
    let (st, o) = api.raw("POST", "/api/v1/orders", Some(&amina_tok), Some(json!({ "customer_name": "Test", "request": req }))).await;
    assert_eq!(st, StatusCode::CREATED, "{o}");
    assert_eq!(o["created_by"], "amina");
    let oid = o["id"].as_str().unwrap().to_string();
    let (_, list) = api.raw("GET", "/api/v1/orders", Some(&fundi), None).await;
    assert_eq!(list[0]["created_by"], "amina"); // everyone sees the shared order list
    let (_, cur) = api.raw("GET", "/api/v1/settings/rules", Some(&owner), None).await;
    let mut priced = cur["rules"].clone();
    priced["stock_price_ksh"] = json!("2300");
    let (st, v) = api.raw("PUT", "/api/v1/settings/rules", Some(&owner), Some(json!({ "rules": priced, "note": "price" }))).await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    assert_eq!(v["changed_by"], "owner");

    // --- PDF tickets: a browser tab can open one quotation, nothing else ---
    let (st, _) = api.raw_bytes(&format!("/api/v1/orders/{oid}/quote.pdf"), None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (_, t) = api.raw("POST", "/api/v1/auth/ticket", Some(&amina_tok), None).await;
    let ticket = t["ticket"].as_str().unwrap().to_string();
    let (st, pdf) = api.raw_bytes(&format!("/api/v1/orders/{oid}/quote.pdf?ticket={ticket}"), None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(pdf.starts_with(b"%PDF"));
    let (st, _) = api.raw_bytes(&format!("/api/v1/orders/{oid}/quote.pdf?ticket=bogus"), None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = api.raw_bytes(&format!("/api/v1/orders/{oid}?ticket={ticket}"), None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED); // a ticket opens PDFs only

    // --- deactivate: existing logins die at once and new ones are refused ---
    let (st, v) = api.raw("PUT", &format!("/api/v1/users/{amina_id}"), Some(&owner), Some(json!({ "active": false }))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (st, _) = api.raw("GET", "/api/v1/orders", Some(&amina_tok), None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = api.raw("POST", "/api/v1/auth/login", None, Some(json!({ "username": "amina", "password": "long enough pw" }))).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = api.raw("PUT", &format!("/api/v1/users/{amina_id}"), Some(&owner), Some(json!({ "active": true, "role": "admin", "new_password": "another good pw" }))).await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = api.raw("POST", "/api/v1/auth/login", None, Some(json!({ "username": "amina", "password": "long enough pw" }))).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED); // admin reset her password
    let (st, _) = api.raw("POST", "/api/v1/auth/login", None, Some(json!({ "username": "amina", "password": "another good pw" }))).await;
    assert_eq!(st, StatusCode::OK);

    // --- the last administrator cannot be removed ---
    let (_, users) = api.raw("GET", "/api/v1/users", Some(&owner), None).await;
    let owner_id = users.as_array().unwrap().iter().find(|u| u["username"] == "owner").unwrap()["id"].as_str().unwrap().to_string();
    let amina_tok = api.login("amina", "another good pw").await;
    // two admins now (owner, amina): owner may step down...
    let (st, _) = api.raw("PUT", &format!("/api/v1/users/{owner_id}"), Some(&amina_tok), Some(json!({ "role": "staff" }))).await;
    assert_eq!(st, StatusCode::OK);
    // ...but amina is now the only one
    let (st, v) = api.raw("PUT", &format!("/api/v1/users/{amina_id}"), Some(&amina_tok), Some(json!({ "active": false }))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST, "{v}");
    let (st, _) = api.raw("PUT", &format!("/api/v1/users/{amina_id}"), Some(&amina_tok), Some(json!({ "role": "staff" }))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // --- change own password: old logins and old password stop working ---
    let (st, _) = api.raw("POST", "/api/v1/auth/password", Some(&fundi), Some(json!({ "current_password": "nope", "new_password": "brand new password" }))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, _) = api.raw("POST", "/api/v1/auth/password", Some(&fundi), Some(json!({ "current_password": PASSWORD, "new_password": "short" }))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let (st, v) = api.raw("POST", "/api/v1/auth/password", Some(&fundi), Some(json!({ "current_password": PASSWORD, "new_password": "brand new password" }))).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let fresh = v["token"].as_str().unwrap().to_string();
    assert_eq!(api.raw("GET", "/api/v1/auth/me", Some(&fundi), None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(api.raw("GET", "/api/v1/auth/me", Some(&fresh), None).await.0, StatusCode::OK);
    let (st, _) = api.raw("POST", "/api/v1/auth/login", None, Some(json!({ "username": "fundi", "password": PASSWORD }))).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // --- logout ---
    assert_eq!(api.raw("POST", "/api/v1/auth/logout", Some(&fresh), None).await.0, StatusCode::OK);
    assert_eq!(api.raw("GET", "/api/v1/auth/me", Some(&fresh), None).await.0, StatusCode::UNAUTHORIZED);

    // --- guessing: five wrong passwords lock the name for a minute, even for the right one ---
    for _ in 0..5 {
        let (st, _) = api.raw("POST", "/api/v1/auth/login", None, Some(json!({ "username": "owner", "password": "guess" }))).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
    }
    let (st, v) = api.raw("POST", "/api/v1/auth/login", None, Some(json!({ "username": "owner", "password": PASSWORD }))).await;
    assert_eq!(st, StatusCode::TOO_MANY_REQUESTS, "{v}");
}


/// A tiny but structurally valid JPEG header (SOF0, 8-bit, 3 components).
fn tiny_jpeg() -> Vec<u8> {
    let mut d = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 17, 8, 0x00, 0x40, 0x00, 0x80, 3];
    for c in 1..=3u8 {
        d.extend_from_slice(&[c, 0x11, 0]);
    }
    d.extend_from_slice(&[0xFF, 0xD9]);
    d
}

/// Letterhead, payment details, default terms and logo.
pub async fn business_suite(store: Arc<dyn Store>) {
    let api = Api::new(store, None).await;
    let (owner, fundi) = (api.login("owner", PASSWORD).await, api.login("fundi", PASSWORD).await);

    let (st, p) = api.raw("GET", "/api/v1/settings/business", Some(&fundi), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(p["company_name"], ""); // nothing is invented
    assert!(p["quote_valid_days"].is_null());
    assert_eq!(api.raw("GET", "/api/v1/settings/business/logo", Some(&fundi), None).await.0, StatusCode::NOT_FOUND);

    let profile = json!({
        "company_name": "Amber Stores Kenya", "address": "Moi Avenue
Nairobi", "phone": "0700 000 000", "email": "sales@example.com",
        "tax_pin": "P000000000X", "payment_details": "M-Pesa Paybill 123456
Account: your name",
        "default_payment_terms": "70% deposit, balance on delivery", "quote_valid_days": 14, "footer_note": "Thank you"
    });
    assert_eq!(api.raw("PUT", "/api/v1/settings/business", Some(&fundi), Some(profile.clone())).await.0, StatusCode::FORBIDDEN);
    assert_eq!(api.raw("PUT", "/api/v1/settings/business", None, Some(profile.clone())).await.0, StatusCode::UNAUTHORIZED);
    for bad in [json!({ "company_name": "two
lines" }), json!({ "quote_valid_days": 0 }), json!({ "quote_valid_days": 400 }), json!({ "footer_note": "x".repeat(1001) }), json!({ "surprise": 1 })] {
        let (st, _) = api.raw("PUT", "/api/v1/settings/business", Some(&owner), Some(bad.clone())).await;
        assert!(st == StatusCode::BAD_REQUEST || st == StatusCode::UNPROCESSABLE_ENTITY, "{bad} -> {st}");
    }
    let (st, v) = api.raw("PUT", "/api/v1/settings/business", Some(&owner), Some(profile.clone())).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (_, p) = api.raw("GET", "/api/v1/settings/business", Some(&fundi), None).await;
    assert_eq!((p["company_name"].as_str(), p["quote_valid_days"].as_u64()), (Some("Amber Stores Kenya"), Some(14)));

    // --- logo: administrators only, JPEG only ---
    let jpeg = tiny_jpeg();
    assert_eq!(api.put_bytes("/api/v1/settings/business/logo", &fundi, jpeg.clone()).await, StatusCode::FORBIDDEN);
    assert_eq!(api.put_bytes("/api/v1/settings/business/logo", &owner, b"GIF89a not a jpeg".to_vec()).await, StatusCode::BAD_REQUEST);
    assert_eq!(api.put_bytes("/api/v1/settings/business/logo", &owner, vec![0xFF; 600 * 1024]).await, StatusCode::BAD_REQUEST);
    assert_eq!(api.put_bytes("/api/v1/settings/business/logo", &owner, jpeg.clone()).await, StatusCode::OK);
    let (st, back) = api.raw_bytes("/api/v1/settings/business/logo", Some(&fundi)).await;
    assert_eq!((st, back), (StatusCode::OK, jpeg.clone()));

    // --- the quotation PDF carries it all; typed terms beat the default ---
    let req = json!({ "windows": [{ "size": "4 x 4 ft", "quantity": 2 }], "view": "full", "options": { "transport_exact_ksh": "500" } });
    let (_, o) = api.raw("POST", "/api/v1/orders", Some(&fundi), Some(json!({ "customer_name": "Mama Wanjiru", "request": req }))).await;
    let oid = o["id"].as_str().unwrap();
    let (st, pdf) = api.raw_bytes(&format!("/api/v1/orders/{oid}/quote.pdf?deposit_percent=50"), Some(&fundi)).await;
    assert_eq!(st, StatusCode::OK);
    let text = String::from_utf8_lossy(&pdf).to_string();
    for want in ["Amber Stores Kenya", "Moi Avenue", "0700 000 000", "PIN: P000000000X", "M-Pesa Paybill 123456", "70% deposit, balance on delivery", "Valid for     : 14 days", "Thank you", "/Im1 Do"] {
        assert!(text.contains(want), "PDF is missing: {want}");
    }
    let (_, pdf) = api.raw_bytes(&format!("/api/v1/orders/{oid}/quote.pdf?payment_terms=Cash%20on%20delivery"), Some(&fundi)).await;
    let text = String::from_utf8_lossy(&pdf).to_string();
    assert!(text.contains("Cash on delivery") && !text.contains("70% deposit"));
    let (_, q) = api.raw("GET", &format!("/api/v1/orders/{oid}/quote"), Some(&fundi), None).await;
    assert_eq!(q["payment_terms"], "70% deposit, balance on delivery");

    // --- removing the logo and clearing the details ---
    assert_eq!(api.raw("DELETE", "/api/v1/settings/business/logo", Some(&fundi), None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(api.raw("DELETE", "/api/v1/settings/business/logo", Some(&owner), None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(api.raw("GET", "/api/v1/settings/business/logo", Some(&fundi), None).await.0, StatusCode::NOT_FOUND);
    let (_, pdf) = api.raw_bytes(&format!("/api/v1/orders/{oid}/quote.pdf"), Some(&fundi)).await;
    assert!(!String::from_utf8_lossy(&pdf).contains("/Im1 Do"));
}
