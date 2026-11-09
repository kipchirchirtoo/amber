//! Expected values below were worked out by hand from the brief's formulas,
//! not produced by the engine.

use amber_engine::*;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

fn win(size: &str, qty: u32) -> WindowSpec {
    WindowSpec::parse(size, qty, None).unwrap()
}

fn calc(order: &Order) -> AluminiumReport {
    calculate_aluminium(order, &Rules::standard()).unwrap()
}

#[test]
fn single_4x4_ft_window() {
    let r = calc(&Order::new(vec![win("4 x 4 ft", 1)]));
    // width = height = 1219.2
    let expect = [
        (Profile::TopTrack, 1, dec!(1199.2)),
        (Profile::BottomTrack, 1, dec!(1199.2)),
        (Profile::SideTrack, 2, dec!(2438.4)),
        (Profile::MeetingProfile, 2, dec!(2378.4)),
        (Profile::LockingProfile, 2, dec!(2378.4)),
        (Profile::Sash, 4, dec!(2518.4)), // 4 x (609.6 + 20)
    ];
    for (p, pieces, mm) in expect {
        let row = r.get(p).unwrap();
        assert_eq!(row.total_pieces, pieces, "{p:?} pieces");
        assert_eq!(row.total_length_mm, mm, "{p:?} mm");
    }
    assert_eq!(r.grand_total_pieces, 12);
    assert_eq!(r.grand_total_length_mm, dec!(12112.0));
}

#[test]
fn brief_example_order_is_consolidated() {
    // 4x4 ft x5, 5x4 ft x3, 2x2 ft x2
    let r = calc(&Order::new(vec![win("4 x 4 ft", 5), win("5 x 4 ft", 3), win("2 x 2 ft", 2)]));
    // top: 5*1199.2 + 3*1504 + 2*589.6
    assert_eq!(r.get(Profile::TopTrack).unwrap().total_length_mm, dec!(11687.2));
    assert_eq!(r.get(Profile::BottomTrack).unwrap().total_length_mm, dec!(11687.2));
    assert_eq!(r.get(Profile::SideTrack).unwrap().total_pieces, 20);
    // hand totals per window type: 12112*5 + 13331.2*3 + 6016*2
    assert_eq!(r.grand_total_length_mm, dec!(112585.6));
    assert_eq!(r.grand_total_pieces, 120);
    // one consolidated table: exactly six profile rows
    assert_eq!(r.profiles.len(), 6);
}

#[test]
fn width_and_height_are_never_swapped() {
    // 5 wide x 4 high: top track from width (1524 - 20), side from height (1219.2)
    let r = calc(&Order::new(vec![win("5 x 4 ft", 1)]));
    assert_eq!(r.get(Profile::TopTrack).unwrap().total_length_mm, dec!(1504));
    assert_eq!(r.get(Profile::SideTrack).unwrap().cuts[0].length_mm, dec!(1219.2));
    // 4 wide x 5 high is a different window
    let r = calc(&Order::new(vec![win("4 x 5 ft", 1)]));
    assert_eq!(r.get(Profile::TopTrack).unwrap().total_length_mm, dec!(1199.2));
    assert_eq!(r.get(Profile::SideTrack).unwrap().cuts[0].length_mm, dec!(1524));
}

#[test]
fn unit_systems_agree() {
    let a = calc(&Order::new(vec![win("1200 x 1500 mm", 1)]));
    let b = calc(&Order::new(vec![win("1.2 x 1.5 m", 1)]));
    assert_eq!(a, b);
    assert_eq!(a.get(Profile::Sash).unwrap().total_length_mm, dec!(2480)); // 4 x 620
}

#[test]
fn small_toilet_window() {
    let r = calc(&Order::new(vec![win("2 x 2 ft", 1).with_label("toilet window")]));
    assert_eq!(r.grand_total_length_mm, dec!(6016.0));
}

#[test]
fn large_window_many_quantities() {
    // 8 x 6 ft, 100 windows: width 2438.4, height 1828.8
    let r = calc(&Order::new(vec![win("8 x 6 ft", 100)]));
    assert_eq!(r.get(Profile::TopTrack).unwrap().total_pieces, 100);
    assert_eq!(r.get(Profile::TopTrack).unwrap().total_length_mm, dec!(241840)); // 100 * 2418.4
    assert_eq!(r.get(Profile::Sash).unwrap().total_pieces, 400);
    assert_eq!(r.get(Profile::Sash).unwrap().total_length_mm, dec!(495680)); // 400 * 1239.2
}

#[test]
fn identical_cuts_are_grouped_across_lines() {
    let r = calc(&Order::new(vec![win("4 x 4 ft", 2), win("4 x 4 ft", 3)]));
    let side = r.get(Profile::SideTrack).unwrap();
    assert_eq!(side.cuts, vec![CutGroup { length_mm: dec!(1219.2), count: 10 }]);
}

#[test]
fn feet_are_exact_and_rounding_is_display_only() {
    let r = calc(&Order::new(vec![win("4 x 4 ft", 1)]));
    let side = r.get(Profile::SideTrack).unwrap();
    assert_eq!(side.total_length_ft, dec!(8)); // 2 x 4 ft, exact
    assert_eq!(round_display(r.grand_total_length_ft), dec!(39.74)); // 12112 / 304.8
    assert_ne!(r.grand_total_length_ft, dec!(39.74)); // stored value untouched
}

#[test]
fn deterministic_and_order_independent() {
    let a = Order::new(vec![win("4 x 4 ft", 5), win("5 x 4 ft", 3), win("2 x 2 ft", 2)]);
    let b = Order::new(vec![win("2 x 2 ft", 2), win("5 x 4 ft", 3), win("4 x 4 ft", 5)]);
    assert_eq!(calc(&a), calc(&a));
    assert_eq!(calc(&a), calc(&b));
}

#[test]
fn fractional_millimetres_stay_exact() {
    // 3.5 ft = 1066.8 mm; sash = 533.4 + 20
    let r = calc(&Order::new(vec![win("3.5 x 3.5 ft", 1)]));
    assert_eq!(r.get(Profile::Sash).unwrap().cuts[0].length_mm, dec!(553.4));
}

#[test]
fn rejects_impossible_inputs() {
    let rules = Rules::standard();
    let run = |o: Order| calculate_aluminium(&o, &rules);

    assert_eq!(run(Order::default()), Err(EngineError::EmptyOrder));
    assert!(matches!(run(Order::new(vec![win("4 x 4 ft", 0)])), Err(EngineError::ZeroQuantity(_))));
    assert!(matches!(
        run(Order::new(vec![WindowSpec::new(Length::from_mm(dec!(-5)), Length::from_mm(dec!(500)), 1)])),
        Err(EngineError::NonPositive { .. })
    ));
    assert!(matches!(
        run(Order::new(vec![WindowSpec::new(Length::from_mm(Decimal::ZERO), Length::from_mm(dec!(500)), 1)])),
        Err(EngineError::NonPositive { .. })
    ));
    // 20 mm wide: top track would be 0 mm long
    assert!(matches!(run(Order::new(vec![win("20 x 500 mm", 1)])), Err(EngineError::CutNotPositive { .. })));
    // 30 mm high: meeting profile would be 0 mm long
    assert!(matches!(run(Order::new(vec![win("500 x 30 mm", 1)])), Err(EngineError::CutNotPositive { .. })));
    assert!(matches!(run(Order::new(vec![win("200 x 1 m", 1)])), Err(EngineError::TooLarge { .. })));
}

#[test]
fn rules_match_the_brief() {
    let r = Rules::standard();
    assert_eq!(r.stock_length_mm, dec!(6400.8));
    assert_eq!(r.stock_price_ksh, dec!(2000));
    assert_eq!(r.cuts.len(), 6);
    let pieces: Vec<u32> = Profile::ALL.iter().map(|p| r.rule_for(*p).unwrap().pieces_per_window).collect();
    assert_eq!(pieces, vec![1, 1, 2, 2, 2, 4]);
}

#[test]
fn report_serialises_as_exact_strings() {
    let r = calc(&Order::new(vec![win("4 x 4 ft", 1)]));
    let json = serde_json::to_string(&r).unwrap();
    assert!(json.contains("\"12112.0\""), "{json}");
    let back: AluminiumReport = serde_json::from_str(&json).unwrap();
    assert_eq!(back, r);
}

#[test]
fn standard_rules_validate_and_bad_rules_do_not() {
    assert!(Rules::standard().validate().is_ok());

    let mut r = Rules::standard();
    r.cuts.pop();
    assert!(r.validate().is_err()); // missing profile

    let mut r = Rules::standard();
    r.transport.last_mut().unwrap().max_windows = Some(99);
    assert!(r.validate().is_err()); // no open-ended tier

    let mut r = Rules::standard();
    r.stock_length_mm = Decimal::ZERO;
    assert!(r.validate().is_err());

    let mut r = Rules::standard();
    r.rubber.rate_per_metre_ksh = dec!(-1);
    assert!(r.validate().is_err());
}

#[test]
fn formulas_differ_ignores_prices_but_catches_deductions() {
    let base = Rules::standard();
    let mut prices = base.clone();
    prices.stock_price_ksh = dec!(2200);
    prices.labour.large_rate_ksh = dec!(800);
    prices.rubber.rate_per_metre_ksh = dec!(33);
    assert!(!base.formulas_differ(&prices));

    let mut cut = base.clone();
    cut.cuts[0].adjust_mm = dec!(-25);
    assert!(base.formulas_differ(&cut));

    let mut panel = base.clone();
    panel.panels.height_deduction_mm = dec!(100);
    assert!(base.formulas_differ(&panel));
}
