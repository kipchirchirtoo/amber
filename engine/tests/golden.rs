//! Golden-file regression tests.
//!
//! Each case's full engine output is stored in tests/golden/<name>.json and
//! compared exactly. These catch ANY unintended change to a number. They do not
//! prove the numbers are right: that is the job of the hand-calculated tests in
//! aluminium.rs, optimiser.rs and production.rs. Only the owner-approved rules
//! may justify regenerating them:
//!
//!     UPDATE_GOLDEN=1 cargo test -p amber_engine --test golden
//!
//! then review the diff before accepting it.

use amber_engine::*;
use rust_decimal_macros::dec;
use std::path::PathBuf;

fn order(lines: &[(&str, u32)]) -> Order {
    Order::new(lines.iter().map(|(s, q)| WindowSpec::parse(s, *q, None).unwrap()).collect())
}

struct Case {
    name: &'static str,
    order: Order,
    opts: CostingOptions,
}

fn cases() -> Vec<Case> {
    let plain = |t| CostingOptions { transport_exact_ksh: t, ..Default::default() };
    vec![
        Case { name: "brief_example_order", order: order(&[("4 x 4 ft", 5), ("5 x 4 ft", 3), ("2 x 2 ft", 2)]), opts: plain(None) },
        Case { name: "single_toilet_window", order: order(&[("2 x 2 ft", 1)]), opts: plain(Some(dec!(300))) },
        Case { name: "large_windows_bulk", order: order(&[("8 x 6 ft", 40), ("6 x 5 ft", 25)]), opts: plain(Some(dec!(1000))) },
        Case {
            name: "metric_and_mixed_units",
            order: order(&[("1200 x 1500 mm", 3), ("1.2 m x 4 ft", 2), ("900 mm x 3 ft", 4)]),
            opts: plain(None),
        },
        Case {
            name: "user_example_10_5_3",
            order: order(&[("4 x 4 ft", 10), ("5 x 4 ft", 5), ("2 x 2 ft", 3)]),
            opts: CostingOptions { include_glass_sheets: true, transport_exact_ksh: Some(dec!(850)), ..Default::default() },
        },
        Case {
            name: "existing_stock_and_kerf",
            order: order(&[("4 x 4 ft", 6), ("3 x 3 ft", 4)]),
            opts: CostingOptions {
                stock: StockConfig {
                    kerf_mm: dec!(3),
                    existing: vec![
                        ExistingStock { profile: Profile::SideTrack, length_mm: dec!(2438.4), quantity: 2, confirmed: true },
                        ExistingStock { profile: Profile::Sash, length_mm: dec!(3000), quantity: 1, confirmed: true },
                        ExistingStock { profile: Profile::TopTrack, length_mm: dec!(5000), quantity: 1, confirmed: false },
                    ],
                },
                transport_exact_ksh: Some(dec!(600)),
                include_glass_sheets: false,
            },
        },
        Case { name: "single_glass_size", order: order(&[("4 x 3 ft", 9)]), opts: CostingOptions { include_glass_sheets: true, ..Default::default() } },
    ]
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden").join(format!("{name}.json"))
}

#[test]
fn engine_output_matches_golden_files() {
    let update = std::env::var("UPDATE_GOLDEN").is_ok();
    for case in cases() {
        let result = calculate_full_costing(&case.order, &Rules::standard(), &case.opts).unwrap();
        let actual = serde_json::to_string_pretty(&result).unwrap() + "\n";
        let path = golden_path(case.name);
        if update {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("missing golden file {path:?}; run with UPDATE_GOLDEN=1 once and review it"));
        assert_eq!(actual.replace("\r\n", "\n"), expected.replace("\r\n", "\n"), "golden mismatch for '{}'", case.name);
    }
}

#[test]
fn every_golden_case_has_unique_name() {
    let mut names: Vec<_> = cases().iter().map(|c| c.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), cases().len());
}
