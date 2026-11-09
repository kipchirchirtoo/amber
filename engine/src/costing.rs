//! Default output (brief section 11) and full costing (section 12).
//!
//! No double counting: aluminium is costed on ONE basis per project total.
//!
//! * `ConsumptionOnly`: aluminium actually used, at KSh 2000/21 per ft.
//! * `WholeBars`: Option A, bars bought new, at the full bar price.
//! * `ExistingPlusPurchase`: Option B, only the bars still to buy. Existing
//!   stock is already owned, so it adds no cash.
//!
//! Rubber and fittings are always consumption cost (never full rolls/boxes).
//! Transport is once per order. Glass sheets are added only when requested.

use crate::aluminium::{calculate_aluminium, AluminiumReport};
use crate::error::Result;
use crate::fittings::{calculate_fittings, FittingsReport};
use crate::glass::{calculate_panels, PanelReport};
use crate::glass_sheets::{calculate_glass_sheets, GlassSheetReport, GlassStatus};
use crate::labour::{calculate_labour, LabourReport};
use crate::optimiser::{optimise_stock, BinSource, PlanStatus, StockConfig, StockPlan};
use crate::order::Order;
use crate::rubber::{calculate_rubber, RubberReport};
use crate::rules::Rules;
use crate::transport::{calculate_transport, TransportCost};
use crate::units::MM_PER_FT;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// What the user sees when they just enter window sizes: aluminium + rubber only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefaultOutput {
    pub aluminium: AluminiumReport,
    pub rubber: RubberReport,
}

pub fn default_output(order: &Order, rules: &Rules) -> Result<DefaultOutput> {
    let panels = calculate_panels(order, rules)?;
    Ok(DefaultOutput {
        aluminium: calculate_aluminium(order, rules)?,
        rubber: calculate_rubber(&panels, rules),
    })
}

#[derive(Debug, Clone, Default)]
pub struct CostingOptions {
    /// Kerf and confirmed existing stock. Empty = Option A only.
    pub stock: StockConfig,
    /// Exact transport amount; `None` leaves transport as an unconfirmed range.
    pub transport_exact_ksh: Option<Decimal>,
    /// Include glass sheet quantities and cost (excluded by default).
    pub include_glass_sheets: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AluminiumBasis {
    ConsumptionOnly,
    WholeBars,
    ExistingPlusPurchase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectTotal {
    pub basis: AluminiumBasis,
    pub aluminium_ksh: Decimal,
    pub rubber_ksh: Decimal,
    pub fittings_ksh: Decimal,
    pub labour_ksh: Decimal,
    /// Included only when glass sheets were requested AND fully quantifiable.
    pub glass_sheets_ksh: Decimal,
    pub transport_low_ksh: Decimal,
    pub transport_high_ksh: Decimal,
    pub total_low_ksh: Decimal,
    pub total_high_ksh: Decimal,
    /// True only when nothing in this total is unconfirmed.
    pub confirmed: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExistingStockUtilisation {
    pub pieces_used: u64,
    /// Total length of cuts taken from existing stock.
    pub cut_length_mm: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FullCosting {
    pub rules_version: String,
    pub total_windows: u64,
    pub aluminium: AluminiumReport,
    /// Cost of the aluminium actually used (proportional KSh 2000 / 21 ft).
    pub aluminium_consumption_cost_ksh: Decimal,
    /// Option A: buy everything new.
    pub stock_option_a: StockPlan,
    /// Option B: present only when confirmed existing stock was supplied.
    pub stock_option_b: Option<StockPlan>,
    pub existing_stock_utilisation: Option<ExistingStockUtilisation>,
    pub panels: PanelReport,
    pub rubber: RubberReport,
    pub fittings: FittingsReport,
    pub labour: LabourReport,
    pub transport: TransportCost,
    pub glass_sheets: Option<GlassSheetReport>,
    pub totals_consumption: ProjectTotal,
    pub totals_whole_bars: ProjectTotal,
    pub totals_existing_plus_purchase: Option<ProjectTotal>,
}

pub fn calculate_full_costing(order: &Order, rules: &Rules, opts: &CostingOptions) -> Result<FullCosting> {
    let aluminium = calculate_aluminium(order, rules)?;
    let panels = calculate_panels(order, rules)?;
    let rubber = calculate_rubber(&panels, rules);
    let fittings = calculate_fittings(order, rules)?;
    let labour = calculate_labour(order, rules)?;
    let transport = calculate_transport(order, rules, opts.transport_exact_ksh)?;
    let glass = if opts.include_glass_sheets { Some(calculate_glass_sheets(order, rules)?) } else { None };

    let option_a_cfg = StockConfig { kerf_mm: opts.stock.kerf_mm, existing: vec![] };
    let stock_a = optimise_stock(&aluminium, rules, &option_a_cfg)?;
    let has_existing = opts.stock.existing.iter().any(|e| e.confirmed && e.quantity > 0);
    let stock_b = if has_existing { Some(optimise_stock(&aluminium, rules, &opts.stock)?) } else { None };

    let rate_per_ft = rules.stock_price_ksh / (rules.stock_length_mm / MM_PER_FT);
    let consumption = aluminium.grand_total_length_ft * rate_per_ft;

    let utilisation = stock_b.as_ref().map(|sb| {
        let (mut pieces, mut len) = (0u64, Decimal::ZERO);
        for p in &sb.profiles {
            for b in p.bins.iter().filter(|b| b.source == BinSource::Existing) {
                pieces += 1;
                len += b.cuts_mm.iter().copied().sum::<Decimal>();
            }
        }
        ExistingStockUtilisation { pieces_used: pieces, cut_length_mm: len }
    });

    let build = |basis: AluminiumBasis, aluminium_ksh: Decimal, plan: Option<&StockPlan>| -> ProjectTotal {
        let mut notes = Vec::new();
        let mut confirmed = true;
        if let Some(p) = plan {
            if p.status != PlanStatus::Verified {
                confirmed = false;
                notes.push("Aluminium stock requirement is unconfirmed: the cutting plan could not be verified.".into());
            }
        }
        let (t_lo, t_hi) = transport.bounds();
        if !transport.is_confirmed() {
            confirmed = false;
            notes.push("Transport is an unconfirmed range: enter the exact amount to confirm.".into());
        }
        let mut glass_ksh = Decimal::ZERO;
        if let Some(g) = &glass {
            match g.cost_ksh {
                Some(c) => {
                    glass_ksh = c;
                    if g.status != GlassStatus::Confirmed {
                        confirmed = false;
                        notes.push("Glass sheet quantity is not fully confirmed by the fundi.".into());
                    }
                }
                None => {
                    confirmed = false;
                    notes.push("Glass sheets excluded from the total: some sizes need fundi confirmation.".into());
                }
            }
        }
        let fixed = aluminium_ksh + rubber.consumption_cost_ksh + fittings.total_cost_ksh + labour.total_ksh + glass_ksh;
        ProjectTotal {
            basis,
            aluminium_ksh,
            rubber_ksh: rubber.consumption_cost_ksh,
            fittings_ksh: fittings.total_cost_ksh,
            labour_ksh: labour.total_ksh,
            glass_sheets_ksh: glass_ksh,
            transport_low_ksh: t_lo,
            transport_high_ksh: t_hi,
            total_low_ksh: fixed + t_lo,
            total_high_ksh: fixed + t_hi,
            confirmed,
            notes,
        }
    };

    Ok(FullCosting {
        rules_version: rules.version.clone(),
        total_windows: order.total_windows(),
        totals_consumption: build(AluminiumBasis::ConsumptionOnly, consumption, None),
        totals_whole_bars: build(AluminiumBasis::WholeBars, stock_a.full_bar_purchase_cost_ksh, Some(&stock_a)),
        totals_existing_plus_purchase: stock_b.as_ref().map(|sb| {
            build(AluminiumBasis::ExistingPlusPurchase, sb.full_bar_purchase_cost_ksh, Some(sb))
        }),
        aluminium,
        aluminium_consumption_cost_ksh: consumption,
        stock_option_a: stock_a,
        stock_option_b: stock_b,
        existing_stock_utilisation: utilisation,
        panels,
        rubber,
        fittings,
        labour,
        transport,
        glass_sheets: glass,
    })
}
