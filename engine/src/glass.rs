//! Glass panel dimensions (brief section 4).
//!
//! Panel width  = window width / 2 - 75 mm
//! Panel height = window height - 110 mm
//!
//! Dimensions are ACTUAL and unrounded. They feed rubber and glass cutting.
//! Glass-order rounding lives in `glass_sheets` and never touches these values.

use crate::error::{EngineError, Result};
use crate::order::Order;
use crate::rules::Rules;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `count` identical panels of `width_mm` x `height_mm`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelGroup {
    pub width_mm: Decimal,
    pub height_mm: Decimal,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelReport {
    /// Identical panels consolidated, widest first.
    pub groups: Vec<PanelGroup>,
    pub total_panels: u64,
}

pub fn calculate_panels(order: &Order, rules: &Rules) -> Result<PanelReport> {
    order.validate()?;
    let p = &rules.panels;
    let mut map: BTreeMap<(Decimal, Decimal), u64> = BTreeMap::new();
    let mut total = 0u64;
    for w in &order.windows {
        let pw = w.width.mm() / Decimal::TWO - p.width_deduction_mm;
        let ph = w.height.mm() - p.height_deduction_mm;
        if pw <= Decimal::ZERO || ph <= Decimal::ZERO {
            return Err(EngineError::PanelNotPositive {
                window: w.display_name(),
                panel_width_mm: pw.to_string(),
                panel_height_mm: ph.to_string(),
            });
        }
        let count = u64::from(p.panels_per_window) * u64::from(w.quantity);
        *map.entry((pw.normalize(), ph.normalize())).or_default() += count;
        total += count;
    }
    let groups = map
        .into_iter()
        .rev()
        .map(|((width_mm, height_mm), count)| PanelGroup { width_mm, height_mm, count })
        .collect();
    Ok(PanelReport { groups, total_panels: total })
}
