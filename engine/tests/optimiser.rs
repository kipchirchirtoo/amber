use amber_engine::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn rules() -> Rules {
    Rules::standard()
}

fn g(length: Decimal, count: u64) -> CutGroup {
    CutGroup { length_mm: length, count }
}

fn plan(groups: &[CutGroup], cfg: &StockConfig) -> ProfilePlan {
    plan_profile(Profile::SideTrack, groups, &rules(), cfg).unwrap()
}

fn existing(profile: Profile, length: Decimal, qty: u32, confirmed: bool) -> ExistingStock {
    ExistingStock { profile, length_mm: length, quantity: qty, confirmed }
}

#[test]
fn total_length_division_is_not_the_purchase_quantity() {
    // 3 x 4000 mm = 12000 mm. 12000 / 6400.8 = 1.87 -> naive answer "2 bars".
    // Only one 4000 mm cut fits per bar, so 3 bars are physically needed.
    let p = plan(&[g(dec!(4000), 3)], &StockConfig::default());
    assert_eq!(p.lower_bound_bars_without_existing, 2);
    assert_eq!(p.new_bars_to_buy, 3);
    assert_eq!(p.status, PlanStatus::Verified);
    assert_eq!(p.full_bar_purchase_cost_ksh, dec!(6000));
}

#[test]
fn exact_fill_needs_one_bar() {
    let p = plan(&[g(dec!(3200.4), 2)], &StockConfig::default());
    assert_eq!(p.new_bars_to_buy, 1);
    assert_eq!(p.bins[0].remaining_mm, dec!(0));
    assert!(p.leftover_mm.is_empty());
}

#[test]
fn kerf_is_zero_by_default_and_counts_when_configured() {
    let cuts = [g(dec!(3200.4), 2)];
    assert_eq!(plan(&cuts, &StockConfig::default()).new_bars_to_buy, 1);
    let with_kerf = StockConfig { kerf_mm: dec!(3), existing: vec![] };
    let p = plan(&cuts, &with_kerf);
    assert_eq!(p.new_bars_to_buy, 2); // 2 x (3200.4 + 3) > 6400.8
    assert_eq!(p.status, PlanStatus::Verified);
}

#[test]
fn cut_longer_than_a_bar_is_unconfirmed_not_hidden() {
    let p = plan(&[g(dec!(7000), 1), g(dec!(1000), 2)], &StockConfig::default());
    assert_eq!(p.status, PlanStatus::Unconfirmed);
    assert_eq!(p.unplaced, vec![g(dec!(7000), 1)]);
    assert!(matches!(p.issues[0], PlanIssue::CutLongerThanStock { .. }));
    assert_eq!(p.new_bars_to_buy, 1); // the placeable cuts still get a plan
}

#[test]
fn cut_that_only_fits_without_kerf_is_unconfirmed() {
    let cfg = StockConfig { kerf_mm: dec!(5), existing: vec![] };
    let p = plan(&[g(dec!(6400.8), 1)], &cfg);
    assert_eq!(p.status, PlanStatus::Unconfirmed);
}

#[test]
fn no_existing_stock_is_assumed() {
    let p = plan(&[g(dec!(1000), 2)], &StockConfig::default());
    assert_eq!(p.existing_available_mm, dec!(0));
    assert_eq!(p.existing_pieces_used, 0);
    assert_eq!(p.shortage_length_mm, dec!(2000));
}

#[test]
fn unconfirmed_existing_stock_is_ignored() {
    let cfg = StockConfig {
        kerf_mm: dec!(0),
        existing: vec![existing(Profile::SideTrack, dec!(5000), 2, false)],
    };
    let p = plan(&[g(dec!(1000), 2)], &cfg);
    assert_eq!(p.existing_pieces_used, 0);
    assert_eq!(p.ignored_unconfirmed_pieces, 2);
    assert_eq!(p.new_bars_to_buy, 1);
}

#[test]
fn existing_stock_for_another_profile_is_not_used() {
    let cfg = StockConfig {
        kerf_mm: dec!(0),
        existing: vec![existing(Profile::Sash, dec!(5000), 5, true)],
    };
    let p = plan(&[g(dec!(1000), 2)], &cfg);
    assert_eq!(p.existing_pieces_used, 0);
}

#[test]
fn brief_example_8ft_of_side_track_covers_one_4x4_window() {
    // 4x4 ft window needs 2 side tracks of 1219.2 mm = 2438.4 mm = exactly 8 ft.
    let order = Order::new(vec![WindowSpec::parse("4 x 4 ft", 1, None).unwrap()]);
    let report = calculate_aluminium(&order, &rules()).unwrap();
    let cfg = StockConfig {
        kerf_mm: dec!(0),
        existing: vec![existing(Profile::SideTrack, dec!(2438.4), 1, true)],
    };
    let sp = optimise_stock(&report, &rules(), &cfg).unwrap();
    let side = sp.profiles.iter().find(|p| p.profile == Profile::SideTrack).unwrap();
    assert_eq!(side.new_bars_to_buy, 0);
    assert_eq!(side.existing_pieces_used, 1);
    assert_eq!(side.shortage_length_mm, dec!(0));
    assert_eq!(side.status, PlanStatus::Verified);
    // the other five profiles still need a bar each
    assert_eq!(sp.total_new_bars, 5);
    assert_eq!(sp.full_bar_purchase_cost_ksh, dec!(10000));
}

#[test]
fn shortage_when_existing_stock_is_too_small() {
    // 8 ft (2438.4) existing, 2 windows need 4878... compute: 4 side cuts x 1219.2 = 4876.8
    let cfg = StockConfig {
        kerf_mm: dec!(0),
        existing: vec![existing(Profile::SideTrack, dec!(2438.4), 1, true)],
    };
    let p = plan(&[g(dec!(1219.2), 4)], &cfg);
    assert_eq!(p.required_length_mm, dec!(4876.8));
    assert_eq!(p.existing_available_mm, dec!(2438.4));
    assert_eq!(p.shortage_length_mm, dec!(2438.4));
    assert_eq!(p.shortage_length_ft, dec!(8));
    // 8 ft x (2000/21) = 16000/21
    assert_eq!(p.shortage_cost_proportional_ksh.round_dp(2), dec!(761.90));
    assert_eq!(p.existing_pieces_used, 1);
    assert_eq!(p.new_bars_to_buy, 1);
}

#[test]
fn useless_short_offcut_is_not_counted_as_a_saving_in_bars() {
    // A 500 mm offcut cannot hold a 1000 mm cut: bars to buy are unchanged,
    // even though the length-based shortage is reduced.
    let cfg = StockConfig {
        kerf_mm: dec!(0),
        existing: vec![existing(Profile::SideTrack, dec!(500), 1, true)],
    };
    let p = plan(&[g(dec!(1000), 2)], &cfg);
    assert_eq!(p.existing_pieces_used, 0);
    assert_eq!(p.new_bars_to_buy, 1);
    assert_eq!(p.shortage_length_mm, dec!(1500));
}

#[test]
fn option_a_vs_option_b_on_a_full_order() {
    let order = Order::new(vec![
        WindowSpec::parse("4 x 4 ft", 5, None).unwrap(),
        WindowSpec::parse("5 x 4 ft", 3, None).unwrap(),
        WindowSpec::parse("2 x 2 ft", 2, None).unwrap(),
    ]);
    let report = calculate_aluminium(&order, &rules()).unwrap();
    let a = optimise_stock(&report, &rules(), &StockConfig::default()).unwrap();
    assert_eq!(a.status, PlanStatus::Verified);

    let top = a.profiles.iter().find(|p| p.profile == Profile::TopTrack).unwrap();
    assert_eq!(top.required_length_mm, dec!(11687.2));
    assert_eq!(top.new_bars_to_buy, 2);
    let side = a.profiles.iter().find(|p| p.profile == Profile::SideTrack).unwrap();
    // 20 cuts of 1219.2: five per bar (6096 <= 6400.8; six = 7315.2 does not fit)
    assert_eq!(side.new_bars_to_buy, 4);

    let cfg = StockConfig {
        kerf_mm: dec!(0),
        existing: vec![existing(Profile::SideTrack, dec!(6400.8), 1, true)],
    };
    let b = optimise_stock(&report, &rules(), &cfg).unwrap();
    let side_b = b.profiles.iter().find(|p| p.profile == Profile::SideTrack).unwrap();
    assert_eq!(side_b.new_bars_to_buy, 3);
    assert_eq!(b.total_new_bars, a.total_new_bars - 1);
    // costs are whole-bar only: nothing double-counted
    assert_eq!(b.full_bar_purchase_cost_ksh, Decimal::from(b.total_new_bars) * dec!(2000));
}

#[test]
fn every_required_cut_is_placed_exactly_once() {
    let order = Order::new(vec![
        WindowSpec::parse("1200 x 1500 mm", 7, None).unwrap(),
        WindowSpec::parse("3 x 2 ft", 4, None).unwrap(),
    ]);
    let report = calculate_aluminium(&order, &rules()).unwrap();
    let sp = optimise_stock(&report, &rules(), &StockConfig { kerf_mm: dec!(3), existing: vec![] }).unwrap();
    for (pp, req) in sp.profiles.iter().zip(&report.profiles) {
        let placed: u64 = pp.bins.iter().map(|b| b.cuts_mm.len() as u64).sum();
        assert_eq!(placed, req.total_pieces);
        assert!(verify_profile_plan(pp, &req.cuts, &rules(), &StockConfig { kerf_mm: dec!(3), existing: vec![] }).is_empty());
    }
}

#[test]
fn verifier_rejects_a_tampered_plan() {
    let cfg = StockConfig::default();
    let required = [g(dec!(4000), 3)];
    let mut p = plan(&required, &cfg);
    // squeeze two 4000 mm cuts into one bar: physically impossible
    let moved = p.bins[1].cuts_mm.pop().unwrap();
    p.bins[0].cuts_mm.push(moved);
    assert!(!verify_profile_plan(&p, &required, &rules(), &cfg).is_empty());

    // drop a cut: requirement no longer met
    let mut p = plan(&required, &cfg);
    p.bins[2].cuts_mm.clear();
    assert!(!verify_profile_plan(&p, &required, &rules(), &cfg).is_empty());

    // claim an existing piece that was never entered
    let mut p = plan(&required, &cfg);
    p.bins[0].source = BinSource::Existing;
    p.bins[0].capacity_mm = dec!(4500);
    assert!(!verify_profile_plan(&p, &required, &rules(), &cfg).is_empty());
}

#[test]
fn invalid_kerf_is_rejected() {
    let neg = StockConfig { kerf_mm: dec!(-1), existing: vec![] };
    assert_eq!(plan_profile(Profile::Sash, &[g(dec!(500), 1)], &rules(), &neg), Err(EngineError::InvalidKerf));
    let huge = StockConfig { kerf_mm: dec!(7000), existing: vec![] };
    assert_eq!(plan_profile(Profile::Sash, &[g(dec!(500), 1)], &rules(), &huge), Err(EngineError::InvalidKerf));
}

#[test]
fn deterministic_and_never_beats_the_lower_bound() {
    // pseudo-random cut lists from a fixed LCG: no external crates, fully reproducible
    let mut s: u64 = 12345;
    let mut next = |m: u64| {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (s >> 33) % m
    };
    for _ in 0..200 {
        let groups: Vec<CutGroup> = (0..(1 + next(5)))
            .map(|_| g(Decimal::from(200 + next(5800)), 1 + next(25)))
            .collect();
        let cfg = StockConfig { kerf_mm: Decimal::from(next(4)), existing: vec![] };
        let a = plan(&groups, &cfg);
        let b = plan(&groups, &cfg);
        assert_eq!(a, b);
        assert_eq!(a.status, PlanStatus::Verified, "{groups:?}");
        assert!(a.new_bars_to_buy >= a.lower_bound_bars_without_existing);
    }
}

#[test]
fn plan_serialises_to_json() {
    let p = plan(&[g(dec!(4000), 3)], &StockConfig::default());
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(serde_json::from_str::<ProfilePlan>(&json).unwrap(), p);
}
