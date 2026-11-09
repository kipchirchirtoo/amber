//! Glass sheet requirements (brief section 5). Optional: never part of the
//! default output.
//!
//! Only the fundi-confirmed yields in `Rules::glass.yields` are used. Anything
//! else is reported as `NeedsFundiConfirmation`: no yield is ever invented.
//!
//! For ORDERING ONLY each window dimension is converted to feet and rounded
//! independently (below .5 down, .5 or above up). Actual production sizes are
//! untouched.
//!
//! Assumptions to confirm with the client: yield sizes are window sizes (not
//! panel sizes), and a rotated size (e.g. 3 wide x 4 high vs 4 x 3) is NOT
//! assumed to share a yield.

use crate::error::Result;
use crate::order::Order;
use crate::rules::Rules;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineStatus {
    Confirmed,
    /// Yield is itself approximate (e.g. "about 4.5 windows per sheet").
    Approximate,
    NeedsFundiConfirmation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlassStatus {
    Confirmed,
    Approximate,
    /// Several sizes: each was given its own sheets. Whether sizes can share a
    /// sheet is unconfirmed, so the total is an upper bound.
    UpperBoundMixedSizes,
    NeedsFundiConfirmation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlassSheetLine {
    pub rounded_width_ft: u32,
    pub rounded_height_ft: u32,
    pub windows: u64,
    pub status: LineStatus,
    /// `None` when no confirmed yield exists.
    pub sheets: Option<u64>,
    /// Spare 2 x 2 ft windows the yield allows. Information only: not applied.
    pub bonus_2x2_windows_not_applied: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlassSheetReport {
    pub lines: Vec<GlassSheetLine>,
    pub status: GlassStatus,
    /// `None` if any line needs fundi confirmation.
    pub total_sheets: Option<u64>,
    pub billed_area_sqft: Option<Decimal>,
    pub cost_ksh: Option<Decimal>,
}

fn round_ft(mm: Decimal) -> u32 {
    (mm / crate::units::MM_PER_FT)
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .to_u32()
        .unwrap_or(0)
}

pub fn calculate_glass_sheets(order: &Order, rules: &Rules) -> Result<GlassSheetReport> {
    order.validate()?;
    let mut by_size: BTreeMap<(u32, u32), u64> = BTreeMap::new();
    for w in &order.windows {
        *by_size.entry((round_ft(w.width.mm()), round_ft(w.height.mm()))).or_default() +=
            u64::from(w.quantity);
    }

    let mut lines = Vec::new();
    for ((rw, rh), windows) in &by_size {
        let y = rules.glass.yields.iter().find(|y| y.width_ft == *rw && y.height_ft == *rh);
        let line = match y {
            Some(y) => {
                let sheets = (windows * u64::from(y.windows_den)).div_ceil(u64::from(y.windows_num));
                GlassSheetLine {
                    rounded_width_ft: *rw,
                    rounded_height_ft: *rh,
                    windows: *windows,
                    status: if y.approximate { LineStatus::Approximate } else { LineStatus::Confirmed },
                    sheets: Some(sheets),
                    bonus_2x2_windows_not_applied: sheets * u64::from(y.bonus_2x2_per_sheet),
                }
            }
            None => GlassSheetLine {
                rounded_width_ft: *rw,
                rounded_height_ft: *rh,
                windows: *windows,
                status: LineStatus::NeedsFundiConfirmation,
                sheets: None,
                bonus_2x2_windows_not_applied: 0,
            },
        };
        lines.push(line);
    }

    let needs_fundi = lines.iter().any(|l| l.status == LineStatus::NeedsFundiConfirmation);
    let approximate = lines.iter().any(|l| l.status == LineStatus::Approximate);
    let status = if needs_fundi {
        GlassStatus::NeedsFundiConfirmation
    } else if lines.len() > 1 {
        GlassStatus::UpperBoundMixedSizes
    } else if approximate {
        GlassStatus::Approximate
    } else {
        GlassStatus::Confirmed
    };
    let total_sheets = if needs_fundi { None } else { Some(lines.iter().filter_map(|l| l.sheets).sum()) };

    Ok(GlassSheetReport {
        lines,
        status,
        total_sheets,
        billed_area_sqft: total_sheets.map(|s| Decimal::from(s) * rules.glass.billing_area_sqft),
        cost_ksh: total_sheets.map(|s| Decimal::from(s) * rules.glass.sheet_price_ksh),
    })
}
