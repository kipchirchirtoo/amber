//! Translates API requests into engine calls. No arithmetic happens here:
//! this file only builds the engine's inputs and chooses which engine report to return.

use crate::error::ApiError;
use amber_engine::*;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_WINDOW_LINES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowInput {
    pub label: Option<String>,
    /// e.g. "4 x 5 ft" (width first). Use this OR width_mm + height_mm.
    pub size: Option<String>,
    pub width_mm: Option<Decimal>,
    pub height_mm: Option<Decimal>,
    pub quantity: u32,
}

/// Existing stock as entered. Give the length as text with a unit ("8 ft",
/// "2438.4 mm", "2.4 m") or as `length_mm`. A bare number is refused.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StockInput {
    pub profile: Profile,
    pub length: Option<String>,
    pub length_mm: Option<Decimal>,
    pub quantity: u32,
    /// Required: the owner must state that this stock is really available.
    pub confirmed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    /// Saw kerf per cut. Zero unless the owner configures it.
    #[serde(default)]
    pub kerf_mm: Decimal,
    /// Only entries with confirmed = true are ever used.
    #[serde(default)]
    pub existing_stock: Vec<StockInput>,
    pub transport_exact_ksh: Option<Decimal>,
    #[serde(default)]
    pub include_glass_sheets: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    /// Aluminium table + rubber only (brief section 11).
    #[default]
    Standard,
    Aluminium,
    Rubber,
    /// Rubber bought in whole 38 m rolls. Only when specifically requested.
    RubberRolls,
    /// Cutting plan / how many 21-ft bars to buy.
    Stock,
    Panels,
    Fittings,
    Labour,
    Transport,
    GlassSheets,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalcRequest {
    pub windows: Vec<WindowInput>,
    pub default_unit: Option<Unit>,
    #[serde(default)]
    pub view: View,
    #[serde(default)]
    pub options: Options,
}

pub fn build_order(req: &CalcRequest) -> std::result::Result<Order, ApiError> {
    if req.windows.len() > MAX_WINDOW_LINES {
        return Err(ApiError::BadRequest(format!("at most {MAX_WINDOW_LINES} window lines per order")));
    }
    let mut windows = Vec::with_capacity(req.windows.len());
    for (i, w) in req.windows.iter().enumerate() {
        let spec = match (&w.size, w.width_mm, w.height_mm) {
            (Some(size), None, None) => WindowSpec::parse(size, w.quantity, req.default_unit)?,
            (None, Some(width), Some(height)) => {
                WindowSpec::new(Length::from_mm(width), Length::from_mm(height), w.quantity)
            }
            _ => {
                return Err(ApiError::BadRequest(format!(
                    "window {}: give either 'size' or both 'width_mm' and 'height_mm'",
                    i + 1
                )))
            }
        };
        windows.push(match &w.label {
            Some(l) => spec.with_label(l.clone()),
            None => spec,
        });
    }
    Ok(Order::new(windows))
}

pub fn run(req: &CalcRequest, rules: &Rules) -> std::result::Result<Value, ApiError> {
    let order = build_order(req)?;
    let o = &req.options;
    let mut existing = Vec::with_capacity(o.existing_stock.len());
    for (i, s) in o.existing_stock.iter().enumerate() {
        let length_mm = match (&s.length, s.length_mm) {
            (Some(text), None) => parse_length(text, None)?.mm(),
            (None, Some(mm)) => mm,
            _ => return Err(ApiError::BadRequest(format!("existing stock {}: give either 'length' or 'length_mm'", i + 1))),
        };
        existing.push(ExistingStock { profile: s.profile, length_mm, quantity: s.quantity, confirmed: s.confirmed });
    }
    let stock = StockConfig { kerf_mm: o.kerf_mm, existing };
    let json = |v: &dyn erased::Ser| v.to_value();

    Ok(match req.view {
        View::Standard => json(&default_output(&order, rules)?),
        View::Aluminium => json(&calculate_aluminium(&order, rules)?),
        View::Rubber => json(&calculate_rubber(&calculate_panels(&order, rules)?, rules)),
        View::RubberRolls => json(&calculate_rubber_rolls(&calculate_panels(&order, rules)?, rules)),
        View::Stock => json(&optimise_stock(&calculate_aluminium(&order, rules)?, rules, &stock)?),
        View::Panels => json(&calculate_panels(&order, rules)?),
        View::Fittings => json(&calculate_fittings(&order, rules)?),
        View::Labour => json(&calculate_labour(&order, rules)?),
        View::Transport => json(&calculate_transport(&order, rules, o.transport_exact_ksh)?),
        View::GlassSheets => json(&calculate_glass_sheets(&order, rules)?),
        View::Full => json(&calculate_full_costing(
            &order,
            rules,
            &CostingOptions { stock, transport_exact_ksh: o.transport_exact_ksh, include_glass_sheets: o.include_glass_sheets },
        )?),
    })
}

mod erased {
    use serde::Serialize;
    use serde_json::Value;

    pub trait Ser {
        fn to_value(&self) -> Value;
    }
    impl<T: Serialize> Ser for T {
        fn to_value(&self) -> Value {
            serde_json::to_value(self).expect("engine reports always serialise")
        }
    }
}
