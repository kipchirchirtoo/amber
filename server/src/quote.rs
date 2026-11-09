//! Quotations. Built ONLY from a saved revision's engine result: this module
//! picks figures out of the snapshot and formats them. It never recomputes
//! aluminium, rubber, labour or fittings, so a quote can never disagree with
//! the saved calculation.

use crate::error::ApiError;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Consumption,
    WholeBars,
    ExistingPlusPurchase,
}

impl Basis {
    pub fn parse(s: &str) -> Result<Self, ApiError> {
        match s {
            "consumption" => Ok(Self::Consumption),
            "whole_bars" => Ok(Self::WholeBars),
            "existing_plus_purchase" => Ok(Self::ExistingPlusPurchase),
            other => Err(ApiError::BadRequest(format!(
                "unknown basis '{other}': use consumption, whole_bars or existing_plus_purchase"
            ))),
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Consumption => "totals_consumption",
            Self::WholeBars => "totals_whole_bars",
            Self::ExistingPlusPurchase => "totals_existing_plus_purchase",
        }
    }
    fn aluminium_label(self) -> &'static str {
        match self {
            Self::Consumption => "Aluminium (actual length used)",
            Self::WholeBars => "Aluminium (whole 21 ft bars)",
            Self::ExistingPlusPurchase => "Aluminium (existing stock + purchase)",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct QuoteLine {
    pub description: String,
    pub amount_ksh: Decimal,
}

/// Owner-supplied pricing. Nothing is priced above cost unless one of these is
/// given (brief section 12).
#[derive(Debug, Clone, Default)]
pub struct Pricing {
    pub selling_price_ksh: Option<Decimal>,
    /// Profit as a percentage of the SELLING price (margin, not markup).
    pub margin_percent: Option<Decimal>,
    /// Owner-supplied unit selling price for each window line, in the same order
    /// as the order's window lines. The server only multiplies and adds.
    pub unit_prices: Option<Vec<Decimal>>,
    pub payment_terms: Option<String>,
}

/// One window line of a quotation priced per window.
#[derive(Debug, Clone, Serialize)]
pub struct PricedWindow {
    pub description: String,
    pub quantity: u64,
    pub unit_price_ksh: Decimal,
    pub line_total_ksh: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct Selling {
    pub cost_ksh: Decimal,
    pub selling_price_ksh: Decimal,
    pub profit_ksh: Decimal,
    pub margin_percent: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct Quote {
    pub selling: Option<Selling>,
    /// Present when the owner gave a unit selling price for each window line.
    pub priced_windows: Option<Vec<PricedWindow>>,
    pub payment_terms: Option<String>,
    pub basis: Basis,
    pub windows: Vec<String>,
    pub lines: Vec<QuoteLine>,
    /// Transport is shown as a line only when confirmed; otherwise it is a range.
    pub transport_note: Option<String>,
    pub total_low_ksh: Decimal,
    pub total_high_ksh: Decimal,
    pub confirmed: bool,
    pub deposit_percent: Option<Decimal>,
    pub deposit_ksh: Option<Decimal>,
    pub balance_ksh: Option<Decimal>,
    pub notes: Vec<String>,
}

pub fn round2(d: Decimal) -> Decimal {
    d.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

fn dec(v: &Value, key: &str) -> Result<Decimal, ApiError> {
    v.get(key)
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<Decimal>().ok())
        .ok_or_else(|| ApiError::BadRequest(format!("saved result is missing '{key}'")))
}

pub fn build(request: &Value, result: &Value, basis: Basis, deposit_percent: Option<Decimal>, pricing: &Pricing) -> Result<Quote, ApiError> {
    let totals = result.get(basis.key()).filter(|t| t.is_object()).ok_or_else(|| {
        ApiError::BadRequest(
            "this revision has no full costing for that basis; save the order from the Full costing view (and add existing stock for existing_plus_purchase)".into(),
        )
    })?;

    let windows = request
        .get("windows")
        .and_then(Value::as_array)
        .map(|ws| {
            ws.iter()
                .map(|w| {
                    let size = w.get("size").and_then(Value::as_str).unwrap_or("?");
                    let q = w.get("quantity").and_then(Value::as_u64).unwrap_or(0);
                    let n = if q == 1 { "window" } else { "windows" };
                    match w.get("label").and_then(Value::as_str).filter(|l| !l.is_empty()) {
                        Some(l) => format!("{size}: {q} {n} ({l})"),
                        None => format!("{size}: {q} {n}"),
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // One entry per window line: (name, quantity), in order, for unit pricing.
    let window_lines: Vec<(String, u64)> = request
        .get("windows")
        .and_then(Value::as_array)
        .map(|ws| {
            ws.iter()
                .map(|w| {
                    let size = match w.get("size").and_then(Value::as_str) {
                        Some(s) => s.to_string(),
                        None => format!(
                            "{} x {} mm",
                            w.get("width_mm").and_then(Value::as_str).unwrap_or("?"),
                            w.get("height_mm").and_then(Value::as_str).unwrap_or("?")
                        ),
                    };
                    let name = match w.get("label").and_then(Value::as_str).filter(|l| !l.is_empty()) {
                        Some(l) => format!("{size} ({l})"),
                        None => size,
                    };
                    (name, w.get("quantity").and_then(Value::as_u64).unwrap_or(0))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut lines = vec![
        QuoteLine { description: basis.aluminium_label().into(), amount_ksh: round2(dec(totals, "aluminium_ksh")?) },
        QuoteLine { description: "Glass-panel rubber".into(), amount_ksh: round2(dec(totals, "rubber_ksh")?) },
        QuoteLine { description: "Fittings and accessories".into(), amount_ksh: round2(dec(totals, "fittings_ksh")?) },
        QuoteLine { description: "Fundi labour".into(), amount_ksh: round2(dec(totals, "labour_ksh")?) },
    ];
    let glass = dec(totals, "glass_sheets_ksh")?;
    if !glass.is_zero() {
        lines.push(QuoteLine { description: "Glass sheets".into(), amount_ksh: round2(glass) });
    }

    let confirmed = totals.get("confirmed").and_then(Value::as_bool).unwrap_or(false);
    let low = round2(dec(totals, "total_low_ksh")?);
    let high = round2(dec(totals, "total_high_ksh")?);
    let t_low = round2(dec(totals, "transport_low_ksh")?);
    let t_high = round2(dec(totals, "transport_high_ksh")?);

    let mut transport_note = None;
    if t_low == t_high {
        lines.push(QuoteLine { description: "Transport".into(), amount_ksh: t_low });
    } else {
        transport_note = Some(format!("Transport is not yet confirmed: between KSh {t_low} and KSh {t_high}."));
    }

    let mut notes: Vec<String> = totals
        .get("notes")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|n| n.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let hundred = Decimal::from(100);
    let mut selling = None;
    let mut priced_windows = None;
    let given = [pricing.selling_price_ksh.is_some(), pricing.margin_percent.is_some(), pricing.unit_prices.is_some()];
    if given.iter().filter(|g| **g).count() > 1 {
        return Err(ApiError::BadRequest("give only one of selling_price_ksh, margin_percent or unit_prices".into()));
    }
    if given.iter().any(|g| *g) {
        if !(confirmed && low == high) {
            return Err(ApiError::BadRequest(
                "a selling price needs a confirmed cost: enter the exact transport amount and save the order again".into(),
            ));
        }
        let price = match (pricing.selling_price_ksh, pricing.margin_percent) {
            _ if pricing.unit_prices.is_some() => {
                let units = pricing.unit_prices.as_deref().unwrap_or_default();
                if units.len() != window_lines.len() {
                    return Err(ApiError::BadRequest(format!(
                        "unit_prices needs one price for each of the {} window line(s); got {}",
                        window_lines.len(),
                        units.len()
                    )));
                }
                let mut priced = Vec::with_capacity(units.len());
                let mut sum = Decimal::ZERO;
                for ((name, qty), unit) in window_lines.iter().zip(units) {
                    if *unit <= Decimal::ZERO {
                        return Err(ApiError::BadRequest(format!("the unit price for '{name}' must be above 0")));
                    }
                    let unit = round2(*unit);
                    let line_total = round2(unit * Decimal::from(*qty));
                    sum += line_total;
                    priced.push(PricedWindow { description: name.clone(), quantity: *qty, unit_price_ksh: unit, line_total_ksh: line_total });
                }
                priced_windows = Some(priced);
                sum
            }
            (Some(p), _) => {
                if p <= Decimal::ZERO {
                    return Err(ApiError::BadRequest("selling_price_ksh must be above 0".into()));
                }
                round2(p)
            }
            (_, Some(m)) => {
                if m < Decimal::ZERO || m >= hundred {
                    return Err(ApiError::BadRequest("margin_percent must be from 0 up to (not including) 100".into()));
                }
                round2(low * hundred / (hundred - m))
            }
            _ => unreachable!(),
        };
        let profit = price - low;
        if profit < Decimal::ZERO {
            notes.push("The selling price is BELOW cost.".into());
        }
        selling = Some(Selling { cost_ksh: low, selling_price_ksh: price, profit_ksh: profit, margin_percent: round2(profit * hundred / price) });
    }

    // The customer pays the selling price when there is one, otherwise the quoted total.
    let payable = selling.as_ref().map(|s| s.selling_price_ksh).unwrap_or(low);
    let (mut deposit_ksh, mut balance_ksh) = (None, None);
    if let Some(p) = deposit_percent {
        if p <= Decimal::ZERO || p > hundred {
            return Err(ApiError::BadRequest("deposit_percent must be above 0 and at most 100".into()));
        }
        if confirmed && low == high {
            let dep = round2(payable * p / hundred);
            deposit_ksh = Some(dep);
            balance_ksh = Some(payable - dep);
        } else {
            notes.push("Deposit and balance are not shown because the total is not confirmed.".into());
        }
    }

    Ok(Quote {
        selling,
        priced_windows,
        payment_terms: pricing.payment_terms.as_ref().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()),
        basis,
        windows,
        lines,
        transport_note,
        total_low_ksh: low,
        total_high_ksh: high,
        confirmed,
        deposit_percent,
        deposit_ksh,
        balance_ksh,
        notes,
    })
}

// ---------------------------------------------------------------- PDF

use crate::store::BusinessProfile;

pub struct Header<'a> {
    pub customer: Option<&'a str>,
    pub reference: &'a str,
    pub revision: i32,
    pub date: &'a str,
    pub business: &'a BusinessProfile,
    pub logo: Option<&'a Logo<'a>>,
}

/// A JPEG that has been checked, ready to embed as-is (no re-encoding).
pub struct Logo<'a> {
    pub data: &'a [u8],
    pub info: JpegInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JpegInfo {
    pub width: u32,
    pub height: u32,
    pub components: u8,
}

pub const MAX_LOGO_BYTES: usize = 500 * 1024;

/// Reads the size of a baseline or progressive JPEG and refuses anything the
/// PDF writer cannot embed safely (not a JPEG, CMYK, not 8-bit, huge, tiny).
pub fn jpeg_info(d: &[u8]) -> Result<JpegInfo, String> {
    if d.len() < 4 || d[0] != 0xFF || d[1] != 0xD8 {
        return Err("the logo must be a JPEG picture (.jpg)".into());
    }
    let mut i = 2;
    while i + 4 <= d.len() {
        if d[i] != 0xFF {
            return Err("the logo file is damaged".into());
        }
        let m = d[i + 1];
        if m == 0xFF {
            i += 1; // padding
            continue;
        }
        if (0xD0..=0xD9).contains(&m) || m == 0x01 {
            i += 2; // markers without a length
            continue;
        }
        let len = u16::from_be_bytes([d[i + 2], d[i + 3]]) as usize;
        if len < 2 || i + 2 + len > d.len() {
            return Err("the logo file is damaged".into());
        }
        let is_sof = matches!(m, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
        if is_sof {
            let seg = &d[i + 4..i + 2 + len];
            if seg.len() < 6 {
                return Err("the logo file is damaged".into());
            }
            let (precision, height, width, comps) = (seg[0], u16::from_be_bytes([seg[1], seg[2]]) as u32, u16::from_be_bytes([seg[3], seg[4]]) as u32, seg[5]);
            if precision != 8 {
                return Err("the logo must be an ordinary 8-bit JPEG".into());
            }
            if comps != 1 && comps != 3 {
                return Err("the logo must be a colour or grey JPEG (CMYK is not supported)".into());
            }
            if width < 16 || height < 16 || width > 4000 || height > 4000 {
                return Err("the logo must be between 16 and 4000 pixels on each side".into());
            }
            return Ok(JpegInfo { width, height, components: comps });
        }
        i += 2 + len;
    }
    Err("the logo file is damaged".into())
}

fn money(d: Decimal) -> String {
    let s = format!("{:.2}", round2(d));
    let (int, frac) = s.split_once('.').unwrap_or((&s, "00"));
    let (neg, digits) = match int.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, int),
    };
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{}{out}.{frac}", if neg { "-" } else { "" })
}

#[derive(Clone, Copy)]
enum Font {
    Regular,
    Bold,
    Mono,
}

impl Font {
    fn name(self) -> &'static str {
        match self {
            Font::Regular => "F1",
            Font::Bold => "F2",
            Font::Mono => "F3",
        }
    }
}

struct Line {
    font: Font,
    size: f32,
    text: String,
}

fn ascii(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii() && !c.is_control() { c } else { '?' }).collect()
}

fn escape(s: &str) -> String {
    ascii(s).replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

/// Wraps at `width` characters on word boundaries (monospace layout).
fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for w in ascii(s).split_whitespace() {
        if !cur.is_empty() && cur.len() + 1 + w.len() > width {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Wraps text that may already contain line breaks, keeping the user's lines.
fn wrap_lines(s: &str, width: usize) -> Vec<String> {
    s.lines().flat_map(|l| if l.trim().is_empty() { vec![String::new()] } else { wrap(l, width) }).collect()
}

const COLS: usize = 80;
const AMT_W: usize = 16;

fn row(desc: &str, amount: &str) -> String {
    let w = COLS - AMT_W;
    let d: String = desc.chars().take(w - 1).collect();
    format!("{d:<w$}{amount:>AMT_W$}")
}

fn layout(q: &Quote, h: &Header) -> Vec<Line> {
    let mut v: Vec<Line> = Vec::new();
    let mut push = |font, size, text: String| v.push(Line { font, size, text });
    let rule = "-".repeat(COLS);
    let b = h.business;

    push(Font::Bold, 14.0, "QUOTATION".into());
    push(Font::Regular, 6.0, String::new());
    push(Font::Mono, 10.0, format!("Quotation ref : {}  (revision {})", h.reference, h.revision));
    push(Font::Mono, 10.0, format!("Date          : {}", h.date));
    if let Some(days) = b.quote_valid_days {
        push(Font::Mono, 10.0, format!("Valid for     : {days} days from the date above"));
    }
    push(Font::Mono, 10.0, format!("Customer      : {}", h.customer.unwrap_or("-")));
    push(Font::Mono, 10.0, String::new());
    push(Font::Bold, 11.0, "Windows".into());
    for w in &q.windows {
        for l in wrap(w, COLS - 2) {
            push(Font::Mono, 10.0, format!("  {l}"));
        }
    }
    push(Font::Mono, 10.0, String::new());
    push(Font::Mono, 10.0, row("Item", "Amount (KSh)"));
    push(Font::Mono, 10.0, rule.clone());
    if let Some(sp) = &q.selling {
        // Customer copy: the price only. Cost and profit stay private.
        match &q.priced_windows {
            Some(pw) => {
                for w in pw {
                    push(Font::Mono, 10.0, row(&format!("{}: {} x {}", w.description, w.quantity, money(w.unit_price_ksh)), &money(w.line_total_ksh)));
                }
            }
            None => push(Font::Mono, 10.0, row("Supply and fitting of windows as listed", &money(sp.selling_price_ksh))),
        }
        push(Font::Mono, 10.0, rule.clone());
        push(Font::Mono, 10.0, row("TOTAL PRICE", &money(sp.selling_price_ksh)));
    } else {
        for l in &q.lines {
            push(Font::Mono, 10.0, row(&l.description, &money(l.amount_ksh)));
        }
        push(Font::Mono, 10.0, rule.clone());
        if q.total_low_ksh == q.total_high_ksh {
            push(Font::Mono, 10.0, row("TOTAL", &money(q.total_low_ksh)));
        } else {
            push(Font::Mono, 10.0, row("TOTAL (low)", &money(q.total_low_ksh)));
            push(Font::Mono, 10.0, row("TOTAL (high)", &money(q.total_high_ksh)));
        }
    }
    if let (Some(p), Some(d), Some(bal)) = (q.deposit_percent, q.deposit_ksh, q.balance_ksh) {
        push(Font::Mono, 10.0, row(&format!("Deposit ({}%)", p.normalize()), &money(d)));
        push(Font::Mono, 10.0, row("Balance", &money(bal)));
    }
    push(Font::Mono, 10.0, String::new());
    if let Some(t) = &q.payment_terms {
        push(Font::Bold, 11.0, "Payment terms".into());
        for l in wrap_lines(t, COLS) {
            push(Font::Mono, 10.0, l);
        }
        push(Font::Mono, 10.0, String::new());
    }
    if !b.payment_details.trim().is_empty() {
        push(Font::Bold, 11.0, "How to pay".into());
        for l in wrap_lines(&b.payment_details, COLS) {
            push(Font::Mono, 10.0, l);
        }
        push(Font::Mono, 10.0, String::new());
    }
    if !q.confirmed {
        push(Font::Bold, 11.0, "UNCONFIRMED - this is an estimate, not a final price.".into());
    }
    if let Some(n) = &q.transport_note {
        for l in wrap(n, COLS) {
            push(Font::Mono, 10.0, l);
        }
    }
    for n in q.notes.iter().filter(|_| q.selling.is_none() || !q.confirmed) {
        for l in wrap(n, COLS) {
            push(Font::Mono, 10.0, l);
        }
    }
    if !b.footer_note.trim().is_empty() {
        push(Font::Mono, 10.0, String::new());
        for l in wrap_lines(&b.footer_note, COLS) {
            push(Font::Regular, 9.0, l);
        }
    }
    v
}

const PAGE_TOP: f32 = 790.0;
const PAGE_BOTTOM: f32 = 60.0;
const LEFT: f32 = 50.0;
const RIGHT: f32 = 545.0;

/// Company name, contact lines and logo across the top of page 1.
/// Returns the drawing operations and the y where the body may start.
fn letterhead(b: &BusinessProfile, logo: Option<&Logo>) -> (String, f32) {
    let name = if b.company_name.trim().is_empty() { "AMBER STORES KENYA" } else { b.company_name.trim() };
    let mut lines: Vec<Line> = vec![Line { font: Font::Bold, size: 18.0, text: name.to_string() }];
    for l in b.address.lines().map(str::trim).filter(|l| !l.is_empty()) {
        lines.push(Line { font: Font::Regular, size: 9.0, text: l.to_string() });
    }
    let contact: Vec<&str> = [b.phone.trim(), b.email.trim(), b.website.trim()].into_iter().filter(|x| !x.is_empty()).collect();
    if !contact.is_empty() {
        lines.push(Line { font: Font::Regular, size: 9.0, text: contact.join("   |   ") });
    }
    if !b.tax_pin.trim().is_empty() {
        lines.push(Line { font: Font::Regular, size: 9.0, text: format!("PIN: {}", b.tax_pin.trim()) });
    }

    let top = PAGE_TOP + 10.0;
    let (mut ops, mut logo_w, mut logo_h) = (String::new(), 0.0_f32, 0.0_f32);
    if let Some(l) = logo {
        let (w, h) = (l.info.width as f32, l.info.height as f32);
        logo_h = 60.0;
        logo_w = logo_h * w / h;
        if logo_w > 160.0 {
            logo_w = 160.0;
            logo_h = logo_w * h / w;
        }
        ops.push_str(&format!("q {logo_w:.1} 0 0 {logo_h:.1} {LEFT} {:.1} cm /Im1 Do Q\n", top - logo_h));
    }
    let x = if logo.is_some() { LEFT + logo_w + 14.0 } else { LEFT };
    let mut y = top;
    for l in &lines {
        y -= l.size * 1.3;
        ops.push_str(&format!("BT /{} {} Tf {x:.1} {y:.1} Td ({}) Tj ET\n", l.font.name(), l.size, escape(&l.text)));
    }
    let bottom = y.min(top - logo_h) - 6.0;
    ops.push_str(&format!("0.8 w {LEFT} {bottom:.1} m {RIGHT} {bottom:.1} l S\n"));
    (ops, bottom - 16.0)
}

pub fn render_pdf(q: &Quote, h: &Header) -> Vec<u8> {
    let lines = layout(q, h);
    let (head_ops, body_top) = letterhead(h.business, h.logo);

    let mut pages: Vec<String> = vec![head_ops];
    let mut y = body_top;
    for l in &lines {
        let lead = l.size * 1.4;
        if y - lead < PAGE_BOTTOM {
            pages.push(String::new());
            y = PAGE_TOP;
        }
        y -= lead;
        if !l.text.is_empty() {
            let page = pages.last_mut().expect("at least one page");
            page.push_str(&format!("BT /{} {} Tf {LEFT} {y:.1} Td ({}) Tj ET\n", l.font.name(), l.size, escape(&l.text)));
        }
    }
    let total = pages.len();
    for (i, p) in pages.iter_mut().enumerate() {
        p.push_str(&format!("BT /F1 8 Tf 480 30 Td (Page {} of {total}) Tj ET\n", i + 1));
    }

    // Object layout: 1 catalog, 2 pages, 3-5 fonts, [6 logo image], then (page, content) pairs.
    let first_page = if h.logo.is_some() { 7 } else { 6 };
    let mut objs: Vec<Vec<u8>> = Vec::new();
    objs.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    let kids: Vec<String> = (0..total).map(|i| format!("{} 0 R", first_page + 2 * i)).collect();
    objs.push(format!("<< /Type /Pages /Kids [{}] /Count {total} >>", kids.join(" ")).into_bytes());
    for f in ["Helvetica", "Helvetica-Bold", "Courier"] {
        objs.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{f} /Encoding /WinAnsiEncoding >>").into_bytes());
    }
    let xobject = if h.logo.is_some() { " /XObject << /Im1 6 0 R >>" } else { "" };
    if let Some(l) = h.logo {
        let cs = if l.info.components == 1 { "/DeviceGray" } else { "/DeviceRGB" };
        let mut o = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace {cs} /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
            l.info.width,
            l.info.height,
            l.data.len()
        )
        .into_bytes();
        o.extend_from_slice(l.data);
        o.extend_from_slice(b"\nendstream");
        objs.push(o);
    }
    for (i, content) in pages.iter().enumerate() {
        objs.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R >>{xobject} >> /Contents {} 0 R >>",
                first_page + 1 + 2 * i
            )
            .into_bytes(),
        );
        objs.push(format!("<< /Length {} >>\nstream\n{}endstream", content.len(), content).into_bytes());
    }

    let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quote() -> Quote {
        Quote {
            selling: None,
            priced_windows: None,
            payment_terms: Some("50% deposit\nbalance on delivery".into()),
            basis: Basis::Consumption,
            windows: vec!["5 x 4 x 4 ft (a (b) \\ c)".into()],
            lines: vec![QuoteLine { description: "Labour".into(), amount_ksh: Decimal::from(6600) }],
            transport_note: None,
            total_low_ksh: Decimal::from(6600),
            total_high_ksh: Decimal::from(6600),
            confirmed: true,
            deposit_percent: None,
            deposit_ksh: None,
            balance_ksh: None,
            notes: vec![],
        }
    }

    /// A minimal valid baseline JPEG header: SOI, SOF0 (8-bit, 40x20, 3 components), EOI.
    fn tiny_jpeg(precision: u8, comps: u8, w: u16, h: u16) -> Vec<u8> {
        let mut d = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46]; // SOI + a short APP0
        d.extend_from_slice(&[0xFF, 0xC0, 0x00, 8 + 3 * comps, precision]);
        d.extend_from_slice(&h.to_be_bytes());
        d.extend_from_slice(&w.to_be_bytes());
        d.push(comps);
        for c in 0..comps {
            d.extend_from_slice(&[c + 1, 0x11, 0]);
        }
        d.extend_from_slice(&[0xFF, 0xD9]);
        d
    }

    fn xref_ok(s: &str, objects: usize) {
        let xref_pos: usize = s.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        let table: Vec<&str> = s[xref_pos..].lines().skip(3).take_while(|l| l.ends_with(" n ")).collect();
        assert_eq!(table.len(), objects);
        for (i, e) in table.iter().enumerate() {
            let off: usize = e[..10].parse().unwrap();
            assert!(s[off..].starts_with(&format!("{} 0 obj", i + 1)), "object {}", i + 1);
        }
    }

    #[test]
    fn money_groups_and_rounds_half_up() {
        assert_eq!(money("47935.314354".parse().unwrap()), "47,935.31");
        assert_eq!(money("0.005".parse().unwrap()), "0.01");
        assert_eq!(money(Decimal::from(1000000)), "1,000,000.00");
    }

    #[test]
    fn pdf_has_valid_structure_and_escapes_text() {
        let biz = BusinessProfile::default();
        let pdf = render_pdf(&quote(), &Header { customer: Some("Wanjiru"), reference: "X", revision: 1, date: "2026-11-07", business: &biz, logo: None });
        let s = String::from_utf8(pdf).unwrap();
        assert!(s.starts_with("%PDF-1.4") && s.trim_end().ends_with("%%EOF"));
        assert!(s.contains("a \\(b\\) \\\\ c"));
        assert!(s.contains("AMBER STORES KENYA")); // default letterhead name
        assert!(s.contains("Page 1 of 1"));
        xref_ok(&s, 7);
    }

    #[test]
    fn letterhead_payment_details_and_logo_are_printed() {
        let biz = BusinessProfile {
            company_name: "Amber Stores Kenya".into(),
            address: "Moi Avenue\nNairobi".into(),
            phone: "0700 000 000".into(),
            email: "sales@example.com".into(),
            tax_pin: "P000000000X".into(),
            payment_details: "M-Pesa Paybill 123456\nAccount: your name".into(),
            quote_valid_days: Some(14),
            footer_note: "Thank you for your business".into(),
            ..Default::default()
        };
        let jpeg = tiny_jpeg(8, 3, 400, 200);
        let info = jpeg_info(&jpeg).unwrap();
        assert_eq!(info, JpegInfo { width: 400, height: 200, components: 3 });
        let logo = Logo { data: &jpeg, info };
        let pdf = render_pdf(&quote(), &Header { customer: None, reference: "R", revision: 2, date: "2026-11-07", business: &biz, logo: Some(&logo) });
        let s = String::from_utf8_lossy(&pdf).to_string();
        for want in ["Moi Avenue", "0700 000 000   |   sales@example.com", "PIN: P000000000X", "M-Pesa Paybill 123456", "Valid for     : 14 days", "balance on delivery", "Thank you for your business", "/Im1 Do", "/DCTDecode"] {
            assert!(s.contains(want), "missing: {want}");
        }
        assert!(pdf.windows(jpeg.len()).any(|w| w == jpeg.as_slice())); // the JPEG bytes are embedded untouched
        // 8 objects: catalog, pages, 3 fonts, image, page, content
        let lossy_ok = s.contains("/Count 1");
        assert!(lossy_ok);
    }

    #[test]
    fn long_quotes_run_onto_a_second_page_with_page_numbers() {
        let mut q = quote();
        q.windows = (0..60).map(|i| format!("{i} x 4 ft: 1 window")).collect();
        let biz = BusinessProfile::default();
        let pdf = render_pdf(&q, &Header { customer: None, reference: "R", revision: 1, date: "d", business: &biz, logo: None });
        let s = String::from_utf8(pdf).unwrap();
        assert!(s.contains("Page 1 of 2") && s.contains("Page 2 of 2"));
        xref_ok(&s, 9);
    }

    #[test]
    fn jpeg_checks_refuse_bad_logos() {
        assert!(jpeg_info(b"not a jpeg at all").is_err());
        assert!(jpeg_info(&tiny_jpeg(12, 3, 100, 100)).is_err()); // not 8-bit
        assert!(jpeg_info(&tiny_jpeg(8, 4, 100, 100)).is_err()); // CMYK
        assert!(jpeg_info(&tiny_jpeg(8, 3, 8, 8)).is_err()); // too small
        assert!(jpeg_info(&tiny_jpeg(8, 3, 5000, 100)).is_err()); // too large
        assert!(jpeg_info(&tiny_jpeg(8, 1, 100, 100)).is_ok()); // grey is fine
        assert!(jpeg_info(&[0xFF, 0xD8, 0xFF]).is_err());
    }
}
