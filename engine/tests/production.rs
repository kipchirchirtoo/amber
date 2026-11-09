//! Glass, rubber, fittings, labour, transport, glass sheets and full costing.
//! Expected values were calculated by hand from the brief.

use amber_engine::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn win(size: &str, qty: u32) -> WindowSpec {
    WindowSpec::parse(size, qty, None).unwrap()
}
fn order(lines: &[(&str, u32)]) -> Order {
    Order::new(lines.iter().map(|(s, q)| win(s, *q)).collect())
}
fn rules() -> Rules {
    Rules::standard()
}
fn brief_order() -> Order {
    order(&[("4 x 4 ft", 5), ("5 x 4 ft", 3), ("2 x 2 ft", 2)])
}

// ---------- glass panels ----------

#[test]
fn panel_dimensions_are_exact_and_unrounded() {
    let r = calculate_panels(&order(&[("4 x 4 ft", 1)]), &rules()).unwrap();
    assert_eq!(r.total_panels, 2);
    assert_eq!(r.groups, vec![PanelGroup { width_mm: dec!(534.6), height_mm: dec!(1109.2), count: 2 }]);
}

#[test]
fn panels_use_width_for_width_and_height_for_height() {
    // 5 wide x 4 high: panel width 1524/2-75 = 687, panel height 1219.2-110 = 1109.2
    let r = calculate_panels(&order(&[("5 x 4 ft", 1)]), &rules()).unwrap();
    assert_eq!((r.groups[0].width_mm, r.groups[0].height_mm), (dec!(687), dec!(1109.2)));
}

#[test]
fn identical_panels_are_consolidated() {
    let r = calculate_panels(&order(&[("4 x 4 ft", 2), ("1219.2 x 1219.2 mm", 3)]), &rules()).unwrap();
    assert_eq!(r.groups.len(), 1);
    assert_eq!(r.groups[0].count, 10);
    assert_eq!(r.total_panels, 10);
}

#[test]
fn tiny_window_with_impossible_panel_is_rejected() {
    // 100 mm high window: panel height would be -10 mm
    let e = calculate_panels(&order(&[("500 x 100 mm", 1)]), &rules());
    assert!(matches!(e, Err(EngineError::PanelNotPositive { .. })));
}

// ---------- rubber ----------

#[test]
fn rubber_single_window() {
    let p = calculate_panels(&order(&[("4 x 4 ft", 1)]), &rules()).unwrap();
    let r = calculate_rubber(&p, &rules());
    // 2 panels x 2 x (534.6 + 1109.2) = 6575.2 mm
    assert_eq!(r.total_mm, dec!(6575.2));
    assert_eq!(r.total_metres, dec!(6.5752));
    assert_eq!(r.rate_per_metre_ksh, dec!(32.80));
    assert_eq!(r.consumption_cost_ksh, dec!(215.66656));
}

#[test]
fn rubber_brief_order_is_consumption_not_full_rolls() {
    let p = calculate_panels(&brief_order(), &rules()).unwrap();
    let r = calculate_rubber(&p, &rules());
    // 32876 + 21554.4 + 5835.2
    assert_eq!(r.total_mm, dec!(60265.6));
    assert_eq!(r.consumption_cost_ksh, dec!(1976.71168));
    // full-roll purchasing exists only as a separate, explicit call
    let rolls = full_roll_purchase(&r, &rules());
    assert_eq!((rolls.rolls, rolls.cost_ksh), (2, dec!(2500)));
}

// ---------- fittings ----------

#[test]
fn fittings_for_ten_windows() {
    let f = calculate_fittings(&order(&[("4 x 4 ft", 10)]), &rules()).unwrap();
    let q = |k| f.get(k).unwrap();
    assert_eq!((q(FittingKind::Roller).total_quantity, q(FittingKind::Roller).total_cost_ksh), (40, dec!(1800)));
    assert_eq!(q(FittingKind::CrescentLock).total_cost_ksh, dec!(1500));
    assert_eq!((q(FittingKind::SilverScrew).total_quantity, q(FittingKind::SilverScrew).total_cost_ksh), (80, dec!(120)));
    assert_eq!((q(FittingKind::BlackMdfScrew).total_quantity, q(FittingKind::BlackMdfScrew).total_cost_ksh), (80, dec!(60)));
    assert_eq!(f.total_cost_ksh, dec!(3480));
    assert_eq!(f.lines.len(), 4); // no screw boxes
}

// ---------- labour ----------

#[test]
fn labour_classification_uses_original_feet() {
    let l = |s: &str| calculate_labour(&order(&[(s, 1)]), &rules()).unwrap().total_ksh;
    assert_eq!(l("2 x 2 ft"), dec!(500)); // exactly 2 x 2 ft is "or smaller"
    assert_eq!(l("1.5 x 2 ft"), dec!(500));
    assert_eq!(l("609.6 x 609.6 mm"), dec!(500));
    assert_eq!(l("2 x 3 ft"), dec!(700)); // one side over 2 ft
    assert_eq!(l("3 x 2 ft"), dec!(700));
    assert_eq!(l("610 x 609.6 mm"), dec!(700)); // 0.4 mm over: no rounding
    assert_eq!(l("4 x 4 ft"), dec!(700));
}

#[test]
fn labour_total_for_brief_order() {
    let l = calculate_labour(&brief_order(), &rules()).unwrap();
    assert_eq!((l.small_windows, l.large_windows), (2, 8));
    assert_eq!(l.total_ksh, dec!(6600)); // 2x500 + 8x700
}

// ---------- transport ----------

#[test]
fn transport_is_once_per_order_and_unconfirmed_without_exact_amount() {
    let t = |n: u32, exact: Option<Decimal>| calculate_transport(&order(&[("4 x 4 ft", n)]), &rules(), exact).unwrap();
    assert_eq!(t(1, None), TransportCost::Unconfirmed { min_ksh: dec!(300), max_ksh: dec!(500) });
    assert_eq!(t(6, None), TransportCost::Unconfirmed { min_ksh: dec!(300), max_ksh: dec!(500) });
    assert_eq!(t(7, None), TransportCost::Unconfirmed { min_ksh: dec!(700), max_ksh: dec!(1000) });
    assert_eq!(t(40, None), TransportCost::Unconfirmed { min_ksh: dec!(700), max_ksh: dec!(1000) });
    assert_eq!(t(40, Some(dec!(850))), TransportCost::Confirmed { amount_ksh: dec!(850) });
    // 3 lines, 6 windows in total: still one band
    let o = order(&[("4 x 4 ft", 2), ("5 x 4 ft", 2), ("2 x 2 ft", 2)]);
    assert_eq!(
        calculate_transport(&o, &rules(), None).unwrap(),
        TransportCost::Unconfirmed { min_ksh: dec!(300), max_ksh: dec!(500) }
    );
    assert!(calculate_transport(&o, &rules(), Some(dec!(-1))).is_err());
}

// ---------- glass sheets ----------

#[test]
fn glass_sheets_use_only_confirmed_yields() {
    let g = |lines: &[(&str, u32)]| calculate_glass_sheets(&order(lines), &rules()).unwrap();

    let r = g(&[("4 x 4 ft", 5)]);
    assert_eq!((r.total_sheets, r.status), (Some(3), GlassStatus::Confirmed)); // 2 per sheet
    assert_eq!(r.cost_ksh, Some(dec!(18000)));
    assert_eq!(r.billed_area_sqft, Some(dec!(114)));
    assert_eq!(r.lines[0].bonus_2x2_windows_not_applied, 3);

    assert_eq!(g(&[("5 x 3 ft", 4)]).total_sheets, Some(2));
    assert_eq!(g(&[("3 x 3 ft", 9)]).total_sheets, Some(3));
    let r = g(&[("4 x 3 ft", 9)]); // 4.5 per sheet, approximate
    assert_eq!((r.total_sheets, r.status), (Some(2), GlassStatus::Approximate));
}

#[test]
fn glass_ordering_rounds_each_dimension_independently() {
    // 4.5 ft = 1371.6 mm rounds UP to 5; 4.4 ft = 1341.12 mm rounds DOWN to 4
    let r = calculate_glass_sheets(&order(&[("4.4 x 4.5 ft", 1)]), &rules()).unwrap();
    assert_eq!((r.lines[0].rounded_width_ft, r.lines[0].rounded_height_ft), (4, 5));
}

#[test]
fn glass_unknown_size_needs_fundi_not_an_invented_yield() {
    let r = calculate_glass_sheets(&order(&[("2 x 2 ft", 3)]), &rules()).unwrap();
    assert_eq!(r.status, GlassStatus::NeedsFundiConfirmation);
    assert_eq!((r.total_sheets, r.cost_ksh), (None, None));
    // a rotated 3 wide x 4 high is not silently treated as 4 x 3
    let r = calculate_glass_sheets(&order(&[("3 x 4 ft", 3)]), &rules()).unwrap();
    assert_eq!(r.status, GlassStatus::NeedsFundiConfirmation);
}

#[test]
fn glass_mixed_sizes_are_an_unconfirmed_upper_bound() {
    let r = calculate_glass_sheets(&order(&[("4 x 4 ft", 2), ("3 x 3 ft", 4)]), &rules()).unwrap();
    assert_eq!(r.status, GlassStatus::UpperBoundMixedSizes);
    assert_eq!(r.total_sheets, Some(2)); // 1 + 1, no sharing assumed
}

// ---------- default output & full costing ----------

#[test]
fn default_output_is_aluminium_and_rubber_only() {
    let d = default_output(&brief_order(), &rules()).unwrap();
    assert_eq!(d.aluminium.grand_total_length_mm, dec!(112585.6));
    assert_eq!(d.rubber.total_metres, dec!(60.2656));
    let json = serde_json::to_value(&d).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 2);
}

#[test]
fn full_costing_brief_order_no_glass_no_existing() {
    let c = calculate_full_costing(&brief_order(), &rules(), &CostingOptions::default()).unwrap();
    assert_eq!(c.total_windows, 10);
    assert!(c.glass_sheets.is_none());
    assert!(c.stock_option_b.is_none());
    assert_eq!(c.labour.total_ksh, dec!(6600));
    assert_eq!(c.fittings.total_cost_ksh, dec!(3480));
    assert!(!c.transport.is_confirmed());

    // aluminium consumption: 112585.6 / 6400.8 bars x 2000 = about 35178.60
    let al = c.aluminium_consumption_cost_ksh;
    assert!(al > dec!(35178.5) && al < dec!(35178.7), "{al}");

    let t = &c.totals_consumption;
    let others = dec!(1976.71168) + dec!(3480) + dec!(6600);
    assert_eq!(t.total_low_ksh, al + others + dec!(700));
    assert_eq!(t.total_high_ksh, al + others + dec!(1000));
    assert!(!t.confirmed);

    // whole-bar basis swaps ONLY the aluminium line
    let w = &c.totals_whole_bars;
    assert_eq!(w.aluminium_ksh, Decimal::from(c.stock_option_a.total_new_bars) * dec!(2000));
    assert_eq!(w.total_low_ksh - w.aluminium_ksh, t.total_low_ksh - t.aluminium_ksh);
    assert!(w.aluminium_ksh >= al.floor()); // buying whole bars never costs less than using the metres
}

#[test]
fn exact_transport_confirms_the_total() {
    let opts = CostingOptions { transport_exact_ksh: Some(dec!(800)), ..Default::default() };
    let c = calculate_full_costing(&brief_order(), &rules(), &opts).unwrap();
    assert!(c.totals_consumption.confirmed);
    assert_eq!(c.totals_consumption.total_low_ksh, c.totals_consumption.total_high_ksh);
}

#[test]
fn existing_stock_adds_option_b_without_double_counting() {
    let opts = CostingOptions {
        stock: StockConfig {
            kerf_mm: dec!(0),
            existing: vec![ExistingStock { profile: Profile::SideTrack, length_mm: dec!(6400.8), quantity: 1, confirmed: true }],
        },
        transport_exact_ksh: Some(dec!(800)),
        ..Default::default()
    };
    let c = calculate_full_costing(&brief_order(), &rules(), &opts).unwrap();
    let b = c.stock_option_b.as_ref().unwrap();
    assert_eq!(b.total_new_bars, c.stock_option_a.total_new_bars - 1);
    let tb = c.totals_existing_plus_purchase.as_ref().unwrap();
    assert_eq!(tb.aluminium_ksh, c.totals_whole_bars.aluminium_ksh - dec!(2000));
    let u = c.existing_stock_utilisation.as_ref().unwrap();
    assert_eq!(u.pieces_used, 1);
    assert_eq!(u.cut_length_mm, dec!(6096)); // 5 cuts of 1219.2
}

#[test]
fn glass_is_included_only_when_requested() {
    let opts = CostingOptions { include_glass_sheets: true, transport_exact_ksh: Some(dec!(500)), ..Default::default() };
    let c = calculate_full_costing(&order(&[("4 x 4 ft", 4)]), &rules(), &opts).unwrap();
    assert_eq!(c.totals_consumption.glass_sheets_ksh, dec!(12000)); // 2 sheets
    assert!(c.totals_consumption.confirmed);

    let plain = calculate_full_costing(&order(&[("4 x 4 ft", 4)]), &rules(), &CostingOptions { transport_exact_ksh: Some(dec!(500)), ..Default::default() }).unwrap();
    assert_eq!(c.totals_consumption.total_low_ksh - plain.totals_consumption.total_low_ksh, dec!(12000));
}

#[test]
fn glass_needing_fundi_is_flagged_not_priced() {
    let opts = CostingOptions { include_glass_sheets: true, transport_exact_ksh: Some(dec!(300)), ..Default::default() };
    let c = calculate_full_costing(&order(&[("2 x 2 ft", 2)]), &rules(), &opts).unwrap();
    assert_eq!(c.totals_consumption.glass_sheets_ksh, dec!(0));
    assert!(!c.totals_consumption.confirmed);
    assert!(c.totals_consumption.notes.iter().any(|n| n.contains("fundi")));
}

#[test]
fn unverifiable_cutting_plan_marks_whole_bar_total_unconfirmed() {
    // 4 m wide window: top track 3980 mm fits; but a 7 m tall one needs a 7000 mm side track
    let c = calculate_full_costing(&order(&[("1000 x 7000 mm", 1)]), &rules(), &CostingOptions { transport_exact_ksh: Some(dec!(300)), ..Default::default() }).unwrap();
    assert_eq!(c.stock_option_a.status, PlanStatus::Unconfirmed);
    assert!(!c.totals_whole_bars.confirmed);
    assert!(c.totals_consumption.confirmed); // consumption basis needs no bars
}

#[test]
fn full_costing_is_deterministic_and_serialises() {
    let o = brief_order();
    let a = calculate_full_costing(&o, &rules(), &CostingOptions::default()).unwrap();
    let b = calculate_full_costing(&o, &rules(), &CostingOptions::default()).unwrap();
    assert_eq!(a, b);
    let json = serde_json::to_string(&a).unwrap();
    assert_eq!(serde_json::from_str::<FullCosting>(&json).unwrap(), a);
}

#[test]
fn full_roll_report_is_separate_from_consumption_costing() {
    // 18 windows need 110.4288 m: 3 rolls of 38 m (114 m) at KSh 1,250, 3.5712 m left over.
    let o = order(&[("4 x 4 ft", 10), ("5 x 4 ft", 5), ("2 x 2 ft", 3)]);
    let panels = calculate_panels(&o, &rules()).unwrap();
    let r = calculate_rubber_rolls(&panels, &rules());
    assert_eq!(r.consumption.total_metres, dec!(110.4288));
    assert_eq!(r.rolls, 3);
    assert_eq!(r.full_roll_cost_ksh, dec!(3750));
    assert_eq!(r.unused_metres, dec!(3.5712));
    // normal costing is still actual metres x KSh 32.80, never the roll price
    assert_eq!(r.consumption.consumption_cost_ksh, dec!(110.4288) * dec!(32.80));
    let full = calculate_full_costing(&o, &rules(), &CostingOptions::default()).unwrap();
    assert_eq!(full.rubber.consumption_cost_ksh, r.consumption.consumption_cost_ksh);
}

#[test]
fn one_small_window_needs_one_whole_roll() {
    let panels = calculate_panels(&order(&[("4 x 4 ft", 1)]), &rules()).unwrap();
    let r = calculate_rubber_rolls(&panels, &rules());
    assert_eq!(r.rolls, 1);
    assert_eq!(r.full_roll_cost_ksh, dec!(1250));
    assert_eq!(r.unused_metres, dec!(38) - dec!(6.5752));
}
