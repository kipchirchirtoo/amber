//! Cutting-stock optimiser and stock-purchase planning (brief section 3).
//!
//! Rules enforced here:
//! * Stock to buy comes from a *physical packing* of individual cuts into bars,
//!   never from total length / bar length (that figure is only reported as a
//!   lower bound).
//! * Existing stock/offcuts are used only when explicitly entered AND marked
//!   `confirmed`. Nothing is ever assumed to be available.
//! * Saw kerf is zero unless configured. When configured, every cut consumes
//!   `kerf_mm` extra (conservative: including the last cut on a bar).
//! * A plan is `Verified` only if an independent checker confirms every cut
//!   fits physically and every required cut is placed. Otherwise the stock
//!   requirement is `Unconfirmed`.
//!
//! The packing heuristic is best-fit-decreasing: valid and physically
//! feasible, but not proven to use the minimum possible number of bars.

use crate::aluminium::{AluminiumReport, CutGroup};
use crate::error::{EngineError, Result};
use crate::rules::{Profile, Rules};
use crate::units::MM_PER_FT;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_CUTS_PER_PROFILE: u64 = 20_000;
pub const MAX_EXISTING_PIECES: u64 = 10_000;

/// Existing aluminium of one length for one profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExistingStock {
    pub profile: Profile,
    pub length_mm: Decimal,
    pub quantity: u32,
    /// Only confirmed stock is ever used. Unconfirmed entries are ignored.
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StockConfig {
    /// Saw kerf in mm per cut. Zero unless the owner configures it.
    pub kerf_mm: Decimal,
    pub existing: Vec<ExistingStock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinSource {
    Existing,
    New,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bin {
    pub source: BinSource,
    pub capacity_mm: Decimal,
    pub cuts_mm: Vec<Decimal>,
    /// Cut lengths plus kerf for every cut.
    pub used_mm: Decimal,
    pub remaining_mm: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Verified,
    Unconfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlanIssue {
    /// A required cut is longer than a whole stock bar (including kerf).
    CutLongerThanStock { length_mm: Decimal, count: u64 },
    /// The independent verifier rejected the plan.
    VerificationFailed { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilePlan {
    pub profile: Profile,
    pub status: PlanStatus,
    pub issues: Vec<PlanIssue>,
    pub total_cuts: u64,
    pub required_length_mm: Decimal,
    /// Confirmed existing stock only.
    pub existing_available_mm: Decimal,
    /// Number of unconfirmed existing pieces that were ignored.
    pub ignored_unconfirmed_pieces: u64,
    /// Length-based shortage: max(0, required - confirmed existing). Indicative
    /// only: existing pieces may be too short for any cut.
    pub shortage_length_mm: Decimal,
    pub shortage_length_ft: Decimal,
    /// shortage_length_ft x (bar price / bar length in ft), e.g. KSh 95.24/ft.
    pub shortage_cost_proportional_ksh: Decimal,
    /// Bars that physically must be bought according to the packing.
    pub new_bars_to_buy: u64,
    pub full_bar_purchase_cost_ksh: Decimal,
    pub existing_pieces_used: u64,
    /// ceil(total placeable length / bar length): a floor, NOT a purchase quantity.
    pub lower_bound_bars_without_existing: u64,
    /// Only bins that hold at least one cut.
    pub bins: Vec<Bin>,
    /// Usable remainders of the bins above, longest first.
    pub leftover_mm: Vec<Decimal>,
    /// Cuts that could not be placed anywhere.
    pub unplaced: Vec<CutGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockPlan {
    pub rules_version: String,
    pub kerf_mm: Decimal,
    pub profiles: Vec<ProfilePlan>,
    pub total_new_bars: u64,
    pub full_bar_purchase_cost_ksh: Decimal,
    /// Verified only if every profile is verified.
    pub status: PlanStatus,
}

struct WorkBin {
    source: BinSource,
    capacity: Decimal,
    cuts: Vec<Decimal>,
    used: Decimal,
}

impl WorkBin {
    fn remaining(&self) -> Decimal {
        self.capacity - self.used
    }
}

fn usable_existing(profile: Profile, cfg: &StockConfig) -> impl Iterator<Item = &ExistingStock> {
    cfg.existing
        .iter()
        .filter(move |e| e.profile == profile && e.confirmed && e.length_mm > Decimal::ZERO && e.quantity > 0)
}

/// Index of the bin in `range` with the least remaining length that still fits `need`.
fn best_fit(bins: &[WorkBin], range: std::ops::Range<usize>, need: Decimal) -> Option<usize> {
    let mut best: Option<usize> = None;
    for i in range {
        let rem = bins[i].remaining();
        if rem >= need && best.is_none_or(|b| rem < bins[b].remaining()) {
            best = Some(i);
        }
    }
    best
}

/// Plan one profile's cuts into existing (confirmed) stock first, then new bars.
pub fn plan_profile(
    profile: Profile,
    required: &[CutGroup],
    rules: &Rules,
    cfg: &StockConfig,
) -> Result<ProfilePlan> {
    let stock = rules.stock_length_mm;
    let kerf = cfg.kerf_mm;
    if kerf < Decimal::ZERO || kerf >= stock {
        return Err(EngineError::InvalidKerf);
    }

    let total_cuts: u64 = required.iter().map(|g| g.count).sum();
    if total_cuts > MAX_CUTS_PER_PROFILE {
        return Err(EngineError::LimitExceeded { what: "cuts per profile".into(), limit: MAX_CUTS_PER_PROFILE });
    }
    let existing_pieces: u64 = usable_existing(profile, cfg).map(|e| u64::from(e.quantity)).sum();
    if existing_pieces > MAX_EXISTING_PIECES {
        return Err(EngineError::LimitExceeded { what: "existing stock pieces".into(), limit: MAX_EXISTING_PIECES });
    }
    let ignored_unconfirmed_pieces: u64 = cfg
        .existing
        .iter()
        .filter(|e| e.profile == profile && !e.confirmed)
        .map(|e| u64::from(e.quantity))
        .sum();

    // Individual cuts, longest first (ties are identical values, so order is irrelevant).
    let mut cuts: Vec<Decimal> = Vec::with_capacity(total_cuts as usize);
    for g in required {
        cuts.extend(std::iter::repeat_n(g.length_mm, g.count as usize));
    }
    cuts.sort_by(|a, b| b.cmp(a));

    let mut bins: Vec<WorkBin> = Vec::new();
    let mut existing_available_mm = Decimal::ZERO;
    for e in usable_existing(profile, cfg) {
        existing_available_mm += e.length_mm * Decimal::from(e.quantity);
        for _ in 0..e.quantity {
            bins.push(WorkBin { source: BinSource::Existing, capacity: e.length_mm, cuts: vec![], used: Decimal::ZERO });
        }
    }
    let n_existing = bins.len();

    let mut unplaced: Vec<Decimal> = Vec::new();
    let mut placeable_total = Decimal::ZERO;
    for cut in cuts.iter().copied() {
        let need = cut + kerf;
        let target = best_fit(&bins, 0..n_existing, need)
            .or_else(|| best_fit(&bins, n_existing..bins.len(), need))
            .or_else(|| {
                if need <= stock {
                    bins.push(WorkBin { source: BinSource::New, capacity: stock, cuts: vec![], used: Decimal::ZERO });
                    Some(bins.len() - 1)
                } else {
                    None
                }
            });
        match target {
            Some(i) => {
                bins[i].cuts.push(cut);
                bins[i].used += need;
            }
            None => unplaced.push(cut),
        }
        if need <= stock {
            placeable_total += need;
        }
    }

    let bins_out: Vec<Bin> = bins
        .into_iter()
        .filter(|b| !b.cuts.is_empty())
        .map(|b| Bin {
            source: b.source,
            capacity_mm: b.capacity,
            remaining_mm: b.capacity - b.used,
            used_mm: b.used,
            cuts_mm: b.cuts,
        })
        .collect();

    let new_bars = bins_out.iter().filter(|b| b.source == BinSource::New).count() as u64;
    let existing_used = bins_out.len() as u64 - new_bars;
    let mut leftover: Vec<Decimal> =
        bins_out.iter().map(|b| b.remaining_mm).filter(|r| *r > Decimal::ZERO).collect();
    leftover.sort_by(|a, b| b.cmp(a));

    let mut unplaced_groups: BTreeMap<Decimal, u64> = BTreeMap::new();
    for u in &unplaced {
        *unplaced_groups.entry(*u).or_default() += 1;
    }
    let unplaced: Vec<CutGroup> = unplaced_groups
        .into_iter()
        .rev()
        .map(|(length_mm, count)| CutGroup { length_mm, count })
        .collect();

    let required_mm: Decimal = required.iter().map(|g| g.length_mm * Decimal::from(g.count)).sum();
    let shortage_mm = (required_mm - existing_available_mm).max(Decimal::ZERO);
    let shortage_ft = shortage_mm / MM_PER_FT;
    let stock_ft = stock / MM_PER_FT;
    let rate_per_ft = rules.stock_price_ksh / stock_ft;
    let lower_bound = (placeable_total / stock).ceil().to_u64().unwrap_or(u64::MAX);

    let mut plan = ProfilePlan {
        profile,
        status: PlanStatus::Verified,
        issues: unplaced
            .iter()
            .map(|g| PlanIssue::CutLongerThanStock { length_mm: g.length_mm, count: g.count })
            .collect(),
        total_cuts,
        required_length_mm: required_mm,
        existing_available_mm,
        ignored_unconfirmed_pieces,
        shortage_length_mm: shortage_mm,
        shortage_length_ft: shortage_ft,
        shortage_cost_proportional_ksh: shortage_ft * rate_per_ft,
        new_bars_to_buy: new_bars,
        full_bar_purchase_cost_ksh: Decimal::from(new_bars) * rules.stock_price_ksh,
        existing_pieces_used: existing_used,
        lower_bound_bars_without_existing: lower_bound,
        bins: bins_out,
        leftover_mm: leftover,
        unplaced,
    };

    for detail in verify_profile_plan(&plan, required, rules, cfg) {
        plan.issues.push(PlanIssue::VerificationFailed { detail });
    }
    if !plan.issues.is_empty() {
        plan.status = PlanStatus::Unconfirmed;
    }
    Ok(plan)
}

/// Independent checker. Uses none of the packing code's bookkeeping: it
/// recomputes every bin's usage from its cuts and compares the placed cuts to
/// the requirement. Returns human-readable failures (empty = plan is sound).
pub fn verify_profile_plan(
    plan: &ProfilePlan,
    required: &[CutGroup],
    rules: &Rules,
    cfg: &StockConfig,
) -> Vec<String> {
    let mut failures = Vec::new();

    // 1. Every bin physically holds its cuts.
    for (i, b) in plan.bins.iter().enumerate() {
        let used: Decimal = b.cuts_mm.iter().map(|c| *c + cfg.kerf_mm).sum();
        if used > b.capacity_mm {
            failures.push(format!("bin {i}: cuts need {used} mm but the piece is {} mm", b.capacity_mm));
        }
        if b.source == BinSource::New && b.capacity_mm != rules.stock_length_mm {
            failures.push(format!("bin {i}: new bar capacity {} mm is not the stock length", b.capacity_mm));
        }
    }

    // 2. Existing pieces used were actually confirmed and available.
    let mut avail: BTreeMap<Decimal, u64> = BTreeMap::new();
    for e in usable_existing(plan.profile, cfg) {
        *avail.entry(e.length_mm).or_default() += u64::from(e.quantity);
    }
    let mut used_existing: BTreeMap<Decimal, u64> = BTreeMap::new();
    for b in plan.bins.iter().filter(|b| b.source == BinSource::Existing) {
        *used_existing.entry(b.capacity_mm).or_default() += 1;
    }
    for (len, n) in &used_existing {
        let have = avail.get(len).copied().unwrap_or(0);
        if *n > have {
            failures.push(format!("plan uses {n} existing pieces of {len} mm but only {have} are confirmed"));
        }
    }

    // 3. Placed + unplaced cuts equal exactly what is required.
    let mut want: BTreeMap<Decimal, i64> = BTreeMap::new();
    for g in required {
        *want.entry(g.length_mm).or_default() += g.count as i64;
    }
    for c in plan.bins.iter().flat_map(|b| b.cuts_mm.iter()) {
        *want.entry(*c).or_default() -= 1;
    }
    for g in &plan.unplaced {
        *want.entry(g.length_mm).or_default() -= g.count as i64;
    }
    for (len, diff) in want {
        if diff != 0 {
            failures.push(format!("cut {len} mm: required minus planned = {diff}"));
        }
    }
    failures
}

/// Plan every profile of an aluminium report.
///
/// * Option A (buy new stock only): pass `StockConfig { existing: vec![], .. }`.
/// * Option B (use existing stock, buy the shortage): pass the confirmed stock.
pub fn optimise_stock(report: &AluminiumReport, rules: &Rules, cfg: &StockConfig) -> Result<StockPlan> {
    let mut profiles = Vec::with_capacity(report.profiles.len());
    for p in &report.profiles {
        profiles.push(plan_profile(p.profile, &p.cuts, rules, cfg)?);
    }
    let total_new_bars = profiles.iter().map(|p| p.new_bars_to_buy).sum();
    let status = if profiles.iter().all(|p| p.status == PlanStatus::Verified) {
        PlanStatus::Verified
    } else {
        PlanStatus::Unconfirmed
    };
    Ok(StockPlan {
        rules_version: rules.version.clone(),
        kerf_mm: cfg.kerf_mm,
        total_new_bars,
        full_bar_purchase_cost_ksh: Decimal::from(total_new_bars) * rules.stock_price_ksh,
        profiles,
        status,
    })
}
