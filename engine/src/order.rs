//! Orders: multiple window sizes with quantities, validated up front.

use crate::error::{EngineError, Result};
use crate::units::{parse_size, Length, Unit};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Anything above 100 m is a typing error, not a window. Also keeps decimal
/// arithmetic far from overflow.
pub const MAX_DIMENSION_MM: u32 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSpec {
    /// Free text, e.g. "toilet window". Not used in any calculation.
    pub label: Option<String>,
    pub width: Length,
    pub height: Length,
    pub quantity: u32,
}

impl WindowSpec {
    pub fn new(width: Length, height: Length, quantity: u32) -> Self {
        WindowSpec { label: None, width, height, quantity }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Parse "4 x 5 ft" (width first) plus a quantity.
    pub fn parse(size: &str, quantity: u32, default_unit: Option<Unit>) -> Result<Self> {
        let s = parse_size(size, default_unit)?;
        Ok(WindowSpec::new(s.width, s.height, quantity))
    }

    pub fn display_name(&self) -> String {
        self.label
            .clone()
            .unwrap_or_else(|| format!("{} x {} mm", self.width.mm(), self.height.mm()))
    }

    pub fn validate(&self) -> Result<()> {
        let name = self.display_name();
        if self.quantity == 0 {
            return Err(EngineError::ZeroQuantity(name));
        }
        for (what, l) in [("width", self.width), ("height", self.height)] {
            if l.mm() <= Decimal::ZERO {
                return Err(EngineError::NonPositive { what: format!("{what} of '{name}'") });
            }
            if l.mm() > Decimal::from(MAX_DIMENSION_MM) {
                return Err(EngineError::TooLarge {
                    what: format!("{what} of '{name}'"),
                    limit_mm: MAX_DIMENSION_MM,
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order {
    pub windows: Vec<WindowSpec>,
}

impl Order {
    pub fn new(windows: Vec<WindowSpec>) -> Self {
        Order { windows }
    }

    pub fn total_windows(&self) -> u64 {
        self.windows.iter().map(|w| u64::from(w.quantity)).sum()
    }

    pub fn validate(&self) -> Result<()> {
        if self.windows.is_empty() {
            return Err(EngineError::EmptyOrder);
        }
        self.windows.iter().try_for_each(WindowSpec::validate)
    }
}
