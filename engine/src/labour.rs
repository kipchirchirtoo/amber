//! Fundi labour (brief section 8), classified from the ORIGINAL window size in
//! feet, never from rounded glass-order sizes.
//!
//! A window is "2 x 2 ft or smaller" only if BOTH sides are <= 2 ft.
//! (Assumption to confirm with the client: a 2 x 3 ft window is "larger".)

use crate::order::Order;
use crate::rules::Rules;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabourReport {
    pub small_windows: u64,
    pub small_rate_ksh: Decimal,
    pub large_windows: u64,
    pub large_rate_ksh: Decimal,
    pub total_ksh: Decimal,
}

pub fn calculate_labour(order: &Order, rules: &Rules) -> crate::error::Result<LabourReport> {
    order.validate()?;
    let l = &rules.labour;
    let (mut small, mut large) = (0u64, 0u64);
    for w in &order.windows {
        let is_small = w.width.ft() <= l.small_max_ft && w.height.ft() <= l.small_max_ft;
        if is_small {
            small += u64::from(w.quantity);
        } else {
            large += u64::from(w.quantity);
        }
    }
    Ok(LabourReport {
        small_windows: small,
        small_rate_ksh: l.small_rate_ksh,
        large_windows: large,
        large_rate_ksh: l.large_rate_ksh,
        total_ksh: Decimal::from(small) * l.small_rate_ksh + Decimal::from(large) * l.large_rate_ksh,
    })
}
