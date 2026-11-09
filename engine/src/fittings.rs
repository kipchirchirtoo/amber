//! Rollers, crescent locks and screws (brief section 7). Consumption costing
//! only: no screw boxes or box prices.

use crate::order::Order;
use crate::rules::{FittingKind, Rules};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FittingLine {
    pub kind: FittingKind,
    pub total_quantity: u64,
    pub unit_price_ksh: Decimal,
    pub total_cost_ksh: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FittingsReport {
    pub lines: Vec<FittingLine>,
    pub total_cost_ksh: Decimal,
}

impl FittingsReport {
    pub fn get(&self, kind: FittingKind) -> Option<&FittingLine> {
        self.lines.iter().find(|l| l.kind == kind)
    }
}

pub fn calculate_fittings(order: &Order, rules: &Rules) -> crate::error::Result<FittingsReport> {
    order.validate()?;
    let windows = order.total_windows();
    let lines: Vec<FittingLine> = rules
        .fittings
        .iter()
        .map(|f| {
            let qty = u64::from(f.per_window) * windows;
            FittingLine {
                kind: f.kind,
                total_quantity: qty,
                unit_price_ksh: f.unit_price_ksh,
                total_cost_ksh: Decimal::from(qty) * f.unit_price_ksh,
            }
        })
        .collect();
    let total = lines.iter().map(|l| l.total_cost_ksh).sum();
    Ok(FittingsReport { lines, total_cost_ksh: total })
}
