//! Measurements and unit parsing. Everything is normalised to millimetres
//! using exact decimal arithmetic (1 ft = 304.8 mm exactly).

use crate::error::{EngineError, Result};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

pub const MM_PER_FT: Decimal = dec!(304.8);
pub const MM_PER_M: Decimal = dec!(1000);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Mm,
    Ft,
    M,
}

impl Unit {
    pub fn mm_per_unit(self) -> Decimal {
        match self {
            Unit::Mm => dec!(1),
            Unit::Ft => MM_PER_FT,
            Unit::M => MM_PER_M,
        }
    }

    pub fn parse(s: &str) -> Result<Unit> {
        match s.trim().to_lowercase().trim_end_matches('.') {
            "mm" | "millimetre" | "millimetres" | "millimeter" | "millimeters" => Ok(Unit::Mm),
            "ft" | "feet" | "foot" | "'" => Ok(Unit::Ft),
            "m" | "metre" | "metres" | "meter" | "meters" => Ok(Unit::M),
            other => Err(EngineError::UnknownUnit(other.to_string())),
        }
    }
}

/// A length stored in millimetres as an exact decimal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Length(Decimal);

impl Length {
    pub fn from_mm(mm: Decimal) -> Self {
        Length(mm)
    }

    pub fn new(value: Decimal, unit: Unit) -> Self {
        Length(value * unit.mm_per_unit())
    }

    pub fn mm(self) -> Decimal {
        self.0
    }

    /// Exact value in feet (not rounded).
    pub fn ft(self) -> Decimal {
        self.0 / MM_PER_FT
    }
}

/// A parsed width x height pair. Width is ALWAYS the first number entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    pub width: Length,
    pub height: Length,
}

/// Split "4.5ft" / "1200 mm" / "1.2" into (number, optional unit).
fn parse_part(part: &str) -> Result<(Decimal, Option<Unit>)> {
    let p = part.trim();
    let split = p
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(p.len());
    let (num, unit) = p.split_at(split);
    if num.is_empty() {
        return Err(EngineError::InvalidNumber(p.to_string()));
    }
    let value =
        Decimal::from_str(num).map_err(|_| EngineError::InvalidNumber(num.to_string()))?;
    let unit = unit.trim();
    let unit = if unit.is_empty() { None } else { Some(Unit::parse(unit)?) };
    Ok((value, unit))
}

/// Parse a single dimension such as "1200 mm", "4ft" or "1.2 m".
pub fn parse_length(text: &str, default_unit: Option<Unit>) -> Result<Length> {
    let (value, unit) = parse_part(text)?;
    let unit = unit
        .or(default_unit)
        .ok_or_else(|| EngineError::MissingUnit(text.trim().to_string()))?;
    Ok(Length::new(value, unit))
}

/// Parse "WIDTH x HEIGHT". Accepts `x`, `X`, `×` and `*` as separators.
///
/// Unit rules (never guessed):
/// * each side may carry its own unit (mixed units, e.g. "1.2 m x 4 ft");
/// * a side without a unit shares the other side's unit ("4 x 5 ft" = 4 ft x 5 ft);
/// * if neither side has a unit, `default_unit` is used, otherwise it is an error.
pub fn parse_size(text: &str, default_unit: Option<Unit>) -> Result<Size> {
    let normalised = text.replace(['×', '*'], "x").replace('X', "x");
    let parts: Vec<&str> = normalised.split('x').collect();
    if parts.len() != 2 {
        return Err(EngineError::MalformedSize(text.trim().to_string()));
    }
    let (w, wu) = parse_part(parts[0])?;
    let (h, hu) = parse_part(parts[1])?;
    let (wu, hu) = match (wu, hu) {
        (Some(a), Some(b)) => (a, b),
        (Some(a), None) => (a, a),
        (None, Some(b)) => (b, b),
        (None, None) => {
            let d = default_unit
                .ok_or_else(|| EngineError::MissingUnit(text.trim().to_string()))?;
            (d, d)
        }
    };
    Ok(Size { width: Length::new(w, wu), height: Length::new(h, hu) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_and_x_are_interchangeable_with_or_without_spaces() {
        for t in ["4 * 4 ft", "4*4 ft", "4x4ft", "4 X 4 ft", "4 x 4 ft", "4*4ft"] {
            assert_eq!(mm(t, None), (dec!(1219.2), dec!(1219.2)), "{t}");
        }
        assert_eq!(mm("1200*1500 mm", None), (dec!(1200), dec!(1500)));
    }


    fn mm(s: &str, d: Option<Unit>) -> (Decimal, Decimal) {
        let z = parse_size(s, d).unwrap();
        (z.width.mm(), z.height.mm())
    }

    #[test]
    fn brief_examples() {
        assert_eq!(mm("1200 × 1500 mm", None), (dec!(1200), dec!(1500)));
        assert_eq!(mm("4 × 5 ft", None), (dec!(1219.2), dec!(1524.0)));
        assert_eq!(mm("1.2 × 1.5 metres", None), (dec!(1200), dec!(1500)));
    }

    #[test]
    fn width_is_always_first() {
        let (w, h) = mm("5 x 4 ft", None);
        assert_eq!((w, h), (dec!(1524.0), dec!(1219.2)));
    }

    #[test]
    fn mixed_units() {
        assert_eq!(mm("1.2m x 4ft", None), (dec!(1200), dec!(1219.2)));
        assert_eq!(mm("900 mm x 3 ft", None), (dec!(900), dec!(914.4)));
    }

    #[test]
    fn default_unit_only_when_no_unit_given() {
        assert_eq!(mm("1200 x 1500", Some(Unit::Mm)), (dec!(1200), dec!(1500)));
        assert!(matches!(parse_size("4 x 5", None), Err(EngineError::MissingUnit(_))));
    }

    #[test]
    fn feet_conversion_is_exact() {
        assert_eq!(Length::new(dec!(21), Unit::Ft).mm(), dec!(6400.8));
        assert_eq!(Length::from_mm(dec!(6400.8)).ft(), dec!(21));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_size("4 x", None).is_err());
        assert!(parse_size("4 x 5 x 6 ft", None).is_err());
        assert!(parse_size("-4 x 5 ft", None).is_err());
        assert!(matches!(parse_size("4 x 5 yd", None), Err(EngineError::UnknownUnit(_))));
        assert!(parse_size("", None).is_err());
    }

    #[test]
    fn single_length() {
        assert_eq!(parse_length("2 ft", None).unwrap().mm(), dec!(609.6));
        assert_eq!(parse_length("8", Some(Unit::Ft)).unwrap().mm(), dec!(2438.4));
    }
}
