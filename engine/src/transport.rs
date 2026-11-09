//! Transport (brief section 9): once per order, never per window. An exact
//! amount must be entered to be confirmed; otherwise the band is shown as an
//! unconfirmed range.

use crate::error::{EngineError, Result};
use crate::order::Order;
use crate::rules::Rules;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum TransportCost {
    Confirmed { amount_ksh: Decimal },
    Unconfirmed { min_ksh: Decimal, max_ksh: Decimal },
}

impl TransportCost {
    /// (low, high) for totals; equal when confirmed.
    pub fn bounds(&self) -> (Decimal, Decimal) {
        match self {
            TransportCost::Confirmed { amount_ksh } => (*amount_ksh, *amount_ksh),
            TransportCost::Unconfirmed { min_ksh, max_ksh } => (*min_ksh, *max_ksh),
        }
    }

    pub fn is_confirmed(&self) -> bool {
        matches!(self, TransportCost::Confirmed { .. })
    }
}

pub fn calculate_transport(
    order: &Order,
    rules: &Rules,
    exact_amount_ksh: Option<Decimal>,
) -> Result<TransportCost> {
    order.validate()?;
    if let Some(amount) = exact_amount_ksh {
        if amount < Decimal::ZERO {
            return Err(EngineError::NegativeAmount);
        }
        return Ok(TransportCost::Confirmed { amount_ksh: amount });
    }
    let windows = order.total_windows();
    let tier = rules
        .transport
        .iter()
        .find(|t| t.max_windows.is_none_or(|m| windows <= m))
        .expect("rules must define an open-ended final transport tier");
    Ok(TransportCost::Unconfirmed { min_ksh: tier.min_ksh, max_ksh: tier.max_ksh })
}
