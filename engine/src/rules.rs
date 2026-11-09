//! The fixed Amber Stores business rules (brief section 2).
//!
//! These are data, not scattered constants, so they can be versioned and
//! audited. `Rules::standard()` is the only constructor: changing a rule is a
//! deliberate act by the administrator, never an implicit one.

use crate::units::Length;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    TopTrack,
    BottomTrack,
    SideTrack,
    MeetingProfile,
    LockingProfile,
    Sash,
}

impl Profile {
    pub const ALL: [Profile; 6] = [
        Profile::TopTrack,
        Profile::BottomTrack,
        Profile::SideTrack,
        Profile::MeetingProfile,
        Profile::LockingProfile,
        Profile::Sash,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Profile::TopTrack => "Top track",
            Profile::BottomTrack => "Bottom track",
            Profile::SideTrack => "Side track",
            Profile::MeetingProfile => "Meeting profile",
            Profile::LockingProfile => "Locking profile",
            Profile::Sash => "Sash",
        }
    }
}

/// Which window dimension a cut is derived from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Width,
    Height,
    HalfWidth,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutRule {
    pub profile: Profile,
    pub basis: Basis,
    /// Added to the basis dimension (negative = deduction), in mm.
    pub adjust_mm: Decimal,
    pub pieces_per_window: u32,
}

impl CutRule {
    pub fn cut_length(&self, width: Length, height: Length) -> Length {
        let base = match self.basis {
            Basis::Width => width.mm(),
            Basis::Height => height.mm(),
            Basis::HalfWidth => width.mm() / dec!(2),
        };
        Length::from_mm(base + self.adjust_mm)
    }
}

/// Glass panel formulas (brief section 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelRules {
    pub panels_per_window: u32,
    /// Panel width = window width / 2 - this.
    pub width_deduction_mm: Decimal,
    /// Panel height = window height - this.
    pub height_deduction_mm: Decimal,
}

/// Rubber (brief section 6). Fitted around glass panels only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubberRules {
    pub rate_per_metre_ksh: Decimal,
    pub roll_metres: Decimal,
    pub roll_price_ksh: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FittingKind {
    Roller,
    CrescentLock,
    SilverScrew,
    BlackMdfScrew,
}

impl FittingKind {
    pub fn label(self) -> &'static str {
        match self {
            FittingKind::Roller => "Rollers",
            FittingKind::CrescentLock => "Crescent locks",
            FittingKind::SilverScrew => "Silver screws",
            FittingKind::BlackMdfScrew => "Black MDF screws",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FittingRule {
    pub kind: FittingKind,
    pub per_window: u32,
    pub unit_price_ksh: Decimal,
}

/// Fundi labour (brief section 8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabourRules {
    /// A window is "small" only if BOTH its width and height are at most this many feet.
    pub small_max_ft: Decimal,
    pub small_rate_ksh: Decimal,
    pub large_rate_ksh: Decimal,
}

/// One transport band, applied to the whole order (brief section 9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportTier {
    /// Inclusive upper bound on total windows; `None` = no upper bound.
    pub max_windows: Option<u64>,
    pub min_ksh: Decimal,
    pub max_ksh: Decimal,
}

/// A fundi-confirmed glass sheet yield (brief section 5). Sizes are whole feet,
/// width first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlassYield {
    pub width_ft: u32,
    pub height_ft: u32,
    /// Windows per sheet = windows_num / windows_den.
    pub windows_num: u32,
    pub windows_den: u32,
    /// Extra 2 x 2 ft windows per sheet (information only, never applied).
    pub bonus_2x2_per_sheet: u32,
    pub approximate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlassRules {
    pub sheet_price_ksh: Decimal,
    pub billing_area_sqft: Decimal,
    pub yields: Vec<GlassYield>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rules {
    /// Identifies the rule set a calculation used (stored with every result).
    pub version: String,
    pub cuts: Vec<CutRule>,
    /// Standard stock bar length: 21 ft = 6,400.8 mm.
    pub stock_length_mm: Decimal,
    /// Purchase price of one stock bar in KSh (same for all six profiles).
    pub stock_price_ksh: Decimal,
    pub panels: PanelRules,
    pub rubber: RubberRules,
    pub fittings: Vec<FittingRule>,
    pub labour: LabourRules,
    /// Ordered from smallest order to largest.
    pub transport: Vec<TransportTier>,
    pub glass: GlassRules,
}

impl Rules {
    /// The rules exactly as specified in the Amber Stores brief.
    pub fn standard() -> Rules {
        let cut = |profile, basis, adjust_mm, pieces_per_window| CutRule {
            profile,
            basis,
            adjust_mm,
            pieces_per_window,
        };
        Rules {
            version: "amber-brief-1.0".to_string(),
            cuts: vec![
                cut(Profile::TopTrack, Basis::Width, dec!(-20), 1),
                cut(Profile::BottomTrack, Basis::Width, dec!(-20), 1),
                cut(Profile::SideTrack, Basis::Height, dec!(0), 2),
                cut(Profile::MeetingProfile, Basis::Height, dec!(-30), 2),
                cut(Profile::LockingProfile, Basis::Height, dec!(-30), 2),
                cut(Profile::Sash, Basis::HalfWidth, dec!(20), 4),
            ],
            stock_length_mm: dec!(6400.8),
            stock_price_ksh: dec!(2000),
            panels: PanelRules {
                panels_per_window: 2,
                width_deduction_mm: dec!(75),
                height_deduction_mm: dec!(110),
            },
            rubber: RubberRules {
                rate_per_metre_ksh: dec!(32.80),
                roll_metres: dec!(38),
                roll_price_ksh: dec!(1250),
            },
            fittings: vec![
                FittingRule { kind: FittingKind::Roller, per_window: 4, unit_price_ksh: dec!(45) },
                FittingRule { kind: FittingKind::CrescentLock, per_window: 1, unit_price_ksh: dec!(150) },
                FittingRule { kind: FittingKind::SilverScrew, per_window: 8, unit_price_ksh: dec!(1.50) },
                FittingRule { kind: FittingKind::BlackMdfScrew, per_window: 8, unit_price_ksh: dec!(0.75) },
            ],
            labour: LabourRules {
                small_max_ft: dec!(2),
                small_rate_ksh: dec!(500),
                large_rate_ksh: dec!(700),
            },
            transport: vec![
                TransportTier { max_windows: Some(6), min_ksh: dec!(300), max_ksh: dec!(500) },
                TransportTier { max_windows: None, min_ksh: dec!(700), max_ksh: dec!(1000) },
            ],
            glass: GlassRules {
                sheet_price_ksh: dec!(6000),
                billing_area_sqft: dec!(38),
                yields: vec![
                    GlassYield { width_ft: 4, height_ft: 4, windows_num: 2, windows_den: 1, bonus_2x2_per_sheet: 1, approximate: false },
                    GlassYield { width_ft: 5, height_ft: 3, windows_num: 3, windows_den: 1, bonus_2x2_per_sheet: 0, approximate: false },
                    GlassYield { width_ft: 3, height_ft: 3, windows_num: 4, windows_den: 1, bonus_2x2_per_sheet: 0, approximate: false },
                    GlassYield { width_ft: 4, height_ft: 3, windows_num: 9, windows_den: 2, bonus_2x2_per_sheet: 0, approximate: true },
                ],
            },
        }
    }

    /// True if the measurement formulas differ: aluminium cut rules or glass
    /// panel deductions. Prices, stock length and labour are NOT formulas.
    /// Changing these must be explicitly authorised by the owner.
    pub fn formulas_differ(&self, other: &Rules) -> bool {
        self.cuts != other.cuts || self.panels != other.panels
    }

    /// Reject rule sets the engine cannot safely run on.
    pub fn validate(&self) -> crate::error::Result<()> {
        use crate::error::EngineError::InvalidRules;
        let bad = |m: &str| Err(InvalidRules(m.to_string()));
        let zero = Decimal::ZERO;

        let mut seen = Profile::ALL.to_vec();
        for c in &self.cuts {
            match seen.iter().position(|p| *p == c.profile) {
                Some(i) => {
                    seen.remove(i);
                }
                None => return bad("each of the six profiles must appear exactly once"),
            }
            if c.pieces_per_window == 0 {
                return bad("pieces per window must be at least 1");
            }
        }
        if !seen.is_empty() {
            return bad("each of the six profiles must appear exactly once");
        }
        if self.stock_length_mm <= zero || self.stock_price_ksh < zero {
            return bad("stock length must be positive and stock price cannot be negative");
        }
        let p = &self.panels;
        if p.panels_per_window == 0 || p.width_deduction_mm < zero || p.height_deduction_mm < zero {
            return bad("panel rules are invalid");
        }
        let r = &self.rubber;
        if r.rate_per_metre_ksh < zero || r.roll_metres <= zero || r.roll_price_ksh < zero {
            return bad("rubber rules are invalid");
        }
        let kinds = [FittingKind::Roller, FittingKind::CrescentLock, FittingKind::SilverScrew, FittingKind::BlackMdfScrew];
        for k in kinds {
            if self.fittings.iter().filter(|f| f.kind == k).count() != 1 {
                return bad("each fitting must appear exactly once");
            }
        }
        if self.fittings.len() != kinds.len() || self.fittings.iter().any(|f| f.unit_price_ksh < zero) {
            return bad("fitting prices cannot be negative");
        }
        let l = &self.labour;
        if l.small_max_ft <= zero || l.small_rate_ksh < zero || l.large_rate_ksh < zero {
            return bad("labour rules are invalid");
        }
        match self.transport.last() {
            Some(t) if t.max_windows.is_none() => {}
            _ => return bad("the last transport tier must have no upper limit"),
        }
        let mut prev = 0u64;
        for (i, t) in self.transport.iter().enumerate() {
            if t.min_ksh < zero || t.min_ksh > t.max_ksh {
                return bad("transport tier amounts are invalid");
            }
            if let Some(m) = t.max_windows {
                if i == self.transport.len() - 1 || m <= prev {
                    return bad("transport tiers must be in strictly increasing order");
                }
                prev = m;
            } else if i != self.transport.len() - 1 {
                return bad("only the last transport tier may have no upper limit");
            }
        }
        let g = &self.glass;
        if g.sheet_price_ksh < zero || g.billing_area_sqft <= zero {
            return bad("glass rules are invalid");
        }
        for (i, y) in g.yields.iter().enumerate() {
            if y.windows_num == 0 || y.windows_den == 0 || y.width_ft == 0 || y.height_ft == 0 {
                return bad("glass yields must be positive");
            }
            if g.yields[..i].iter().any(|o| o.width_ft == y.width_ft && o.height_ft == y.height_ft) {
                return bad("duplicate glass yield size");
            }
        }
        Ok(())
    }

    pub fn rule_for(&self, profile: Profile) -> Option<&CutRule> {
        self.cuts.iter().find(|c| c.profile == profile)
    }
}
