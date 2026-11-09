//! Rubber (brief section 6). Fitted around all four sides of each glass panel,
//! never around the aluminium frame.

use crate::glass::PanelReport;
use crate::rules::Rules;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubberReport {
    pub total_mm: Decimal,
    pub total_metres: Decimal,
    pub rate_per_metre_ksh: Decimal,
    /// Actual metres x rate. Exact; round only for display.
    pub consumption_cost_ksh: Decimal,
}

/// Normal costing: actual consumption only.
pub fn calculate_rubber(panels: &PanelReport, rules: &Rules) -> RubberReport {
    let total_mm: Decimal = panels
        .groups
        .iter()
        .map(|g| Decimal::TWO * (g.width_mm + g.height_mm) * Decimal::from(g.count))
        .sum();
    let metres = total_mm / Decimal::from(1000);
    RubberReport {
        total_mm,
        total_metres: metres,
        rate_per_metre_ksh: rules.rubber.rate_per_metre_ksh,
        consumption_cost_ksh: metres * rules.rubber.rate_per_metre_ksh,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullRollPurchase {
    pub rolls: u64,
    pub cost_ksh: Decimal,
}

/// ONLY for when full-roll purchasing is specifically requested.
pub fn full_roll_purchase(report: &RubberReport, rules: &Rules) -> FullRollPurchase {
    let rolls = (report.total_metres / rules.rubber.roll_metres)
        .ceil()
        .to_u64()
        .unwrap_or(u64::MAX);
    FullRollPurchase { rolls, cost_ksh: Decimal::from(rolls) * rules.rubber.roll_price_ksh }
}

/// The report for the specific request "full-roll purchasing" (brief section 6).
/// Normal costing never uses this: it charges actual metres only. Everything
/// here is computed by the engine; the roll size and price come from the rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubberRollReport {
    /// The consumption figures, for comparison.
    pub consumption: RubberReport,
    pub roll_metres: Decimal,
    pub roll_price_ksh: Decimal,
    /// Whole rolls needed to cover the consumption.
    pub rolls: u64,
    pub full_roll_cost_ksh: Decimal,
    /// Metres left over on the last roll after the order is fitted.
    pub unused_metres: Decimal,
}

pub fn calculate_rubber_rolls(panels: &PanelReport, rules: &Rules) -> RubberRollReport {
    let consumption = calculate_rubber(panels, rules);
    let purchase = full_roll_purchase(&consumption, rules);
    let bought = Decimal::from(purchase.rolls) * rules.rubber.roll_metres;
    RubberRollReport {
        roll_metres: rules.rubber.roll_metres,
        roll_price_ksh: rules.rubber.roll_price_ksh,
        rolls: purchase.rolls,
        full_roll_cost_ksh: purchase.cost_ksh,
        unused_metres: bought - consumption.total_metres,
        consumption,
    }
}
