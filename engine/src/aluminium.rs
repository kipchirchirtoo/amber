//! Consolidated aluminium requirements (brief section 2).
//!
//! This is *required length only*. It deliberately says nothing about how many
//! 21-ft bars to buy: that needs the cutting optimiser, because total length
//! divided by bar length is not a valid purchase quantity. Each profile
//! therefore also carries its list of individual cuts, grouped by length,
//! which is the optimiser's input.

use crate::error::{EngineError, Result};
use crate::order::Order;
use crate::rules::{Profile, Rules};
use crate::units::Length;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `count` pieces that are each `length_mm` long.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutGroup {
    pub length_mm: Decimal,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileRequirement {
    pub profile: Profile,
    pub total_pieces: u64,
    /// Exact, unrounded.
    pub total_length_mm: Decimal,
    /// Exact, unrounded. Use [`round_display`] for screens and PDFs only.
    pub total_length_ft: Decimal,
    /// Individual cuts, longest first.
    pub cuts: Vec<CutGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AluminiumReport {
    pub rules_version: String,
    /// One row per profile, in the standard order.
    pub profiles: Vec<ProfileRequirement>,
    pub grand_total_pieces: u64,
    pub grand_total_length_mm: Decimal,
    pub grand_total_length_ft: Decimal,
}

impl AluminiumReport {
    pub fn get(&self, profile: Profile) -> Option<&ProfileRequirement> {
        self.profiles.iter().find(|p| p.profile == profile)
    }
}

/// Rounding for presentation only (2 decimal places). Never feed the result
/// back into a calculation.
pub fn round_display(v: Decimal) -> Decimal {
    v.round_dp(2)
}

pub fn calculate_aluminium(order: &Order, rules: &Rules) -> Result<AluminiumReport> {
    order.validate()?;

    let mut profiles = Vec::with_capacity(rules.cuts.len());
    let mut grand_pieces = 0u64;
    let mut grand_mm = Decimal::ZERO;

    for rule in &rules.cuts {
        let mut groups: BTreeMap<Decimal, u64> = BTreeMap::new();
        let mut pieces = 0u64;
        let mut total_mm = Decimal::ZERO;

        for w in &order.windows {
            let cut = rule.cut_length(w.width, w.height);
            if cut.mm() <= Decimal::ZERO {
                return Err(EngineError::CutNotPositive {
                    window: w.display_name(),
                    profile: rule.profile.label().to_string(),
                    width_mm: w.width.mm().to_string(),
                    height_mm: w.height.mm().to_string(),
                    cut_mm: cut.mm().to_string(),
                });
            }
            let count = u64::from(rule.pieces_per_window) * u64::from(w.quantity);
            *groups.entry(cut.mm().normalize()).or_default() += count;
            pieces += count;
            total_mm += cut.mm() * Decimal::from(count);
        }

        let cuts = groups
            .into_iter()
            .rev()
            .map(|(length_mm, count)| CutGroup { length_mm, count })
            .collect();

        grand_pieces += pieces;
        grand_mm += total_mm;
        profiles.push(ProfileRequirement {
            profile: rule.profile,
            total_pieces: pieces,
            total_length_mm: total_mm,
            total_length_ft: Length::from_mm(total_mm).ft(),
            cuts,
        });
    }

    Ok(AluminiumReport {
        rules_version: rules.version.clone(),
        profiles,
        grand_total_pieces: grand_pieces,
        grand_total_length_mm: grand_mm,
        grand_total_length_ft: Length::from_mm(grand_mm).ft(),
    })
}
