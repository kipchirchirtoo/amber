//! `amber`: try the calculation engine from a terminal.
//!
//! amber "4 x 4 ft:5" "5 x 4 ft:3" "2 x 2 ft:2"            default output
//! amber ... --full [--transport 800] [--glass] [--kerf 3]  full costing
//! amber ... --stock "side_track=8ft*1"                     use confirmed existing stock
//! amber ... --json                                         raw engine JSON
//!
//! Each window is SIZE:QUANTITY (width first). Every number printed comes from
//! amber_engine; this file only parses arguments and lays out text.

use amber_engine::*;
use rust_decimal::Decimal;
use std::str::FromStr;

const USAGE: &str = "usage: amber \"WxH unit:QTY\" ... [--full] [--glass] [--transport KSH] [--kerf MM] \
[--stock \"profile=LENGTH*QTY\"]... [--default-unit mm|ft|m] [--json]";

fn fail(msg: &str) -> ! {
    eprintln!("error: {msg}\n{USAGE}");
    std::process::exit(2)
}

fn money(d: Decimal) -> String {
    format!("{:.2}", d.round_dp(2))
}

fn dec_arg(flag: &str, v: Option<String>) -> Decimal {
    let v = v.unwrap_or_else(|| fail(&format!("{flag} needs a value")));
    Decimal::from_str(&v).unwrap_or_else(|_| fail(&format!("{flag}: '{v}' is not a number")))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let (mut full, mut glass, mut json) = (false, false, false);
    let (mut transport, mut kerf) = (None, Decimal::ZERO);
    let mut default_unit = None;
    let mut windows = Vec::new();
    let mut stock_args = Vec::new();

    while let Some(a) = args.next() {
        match a.as_str() {
            "--full" => full = true,
            "--glass" => glass = true,
            "--json" => json = true,
            "--transport" => transport = Some(dec_arg("--transport", args.next())),
            "--kerf" => kerf = dec_arg("--kerf", args.next()),
            "--stock" => stock_args.push(args.next().unwrap_or_else(|| fail("--stock needs a value"))),
            "--default-unit" => {
                let u = args.next().unwrap_or_else(|| fail("--default-unit needs a value"));
                default_unit = Some(Unit::parse(&u).unwrap_or_else(|e| fail(&e.to_string())));
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return;
            }
            s if s.starts_with("--") => fail(&format!("unknown option {s}")),
            _ => windows.push(a),
        }
    }

    let rules = Rules::standard();
    let order = build_order(&windows, default_unit);
    let existing: Vec<ExistingStock> = stock_args.iter().map(|s| parse_stock(s)).collect();
    // Anything typed on the command line is the user explicitly confirming it.
    let stock = StockConfig { kerf_mm: kerf, existing };

    if full || glass || transport.is_some() || !stock.existing.is_empty() {
        let opts = CostingOptions { stock, transport_exact_ksh: transport, include_glass_sheets: glass };
        let c = calculate_full_costing(&order, &rules, &opts).unwrap_or_else(|e| fail(&e.to_string()));
        if json {
            println!("{}", serde_json::to_string_pretty(&c).unwrap());
        } else {
            print_aluminium(&c.aluminium);
            print_rubber(&c.rubber);
            print_full(&c);
        }
    } else {
        let d = default_output(&order, &rules).unwrap_or_else(|e| fail(&e.to_string()));
        if json {
            println!("{}", serde_json::to_string_pretty(&d).unwrap());
        } else {
            print_aluminium(&d.aluminium);
            print_rubber(&d.rubber);
        }
    }
}

fn build_order(lines: &[String], default_unit: Option<Unit>) -> Order {
    if lines.is_empty() {
        fail("give at least one window, e.g. \"4 x 4 ft:5\"");
    }
    let windows = lines
        .iter()
        .map(|l| {
            let (size, qty) = l.rsplit_once(':').unwrap_or_else(|| fail(&format!("'{l}' needs :QUANTITY")));
            let qty: u32 = qty.trim().parse().unwrap_or_else(|_| fail(&format!("'{qty}' is not a quantity")));
            WindowSpec::parse(size, qty, default_unit).unwrap_or_else(|e| fail(&e.to_string()))
        })
        .collect();
    Order::new(windows)
}

fn parse_stock(s: &str) -> ExistingStock {
    let (name, rest) = s.split_once('=').unwrap_or_else(|| fail(&format!("--stock '{s}': expected profile=LENGTH*QTY")));
    let profile: Profile = serde_json::from_str(&format!("\"{}\"", name.trim()))
        .unwrap_or_else(|_| fail(&format!("unknown profile '{name}' (top_track, bottom_track, side_track, meeting_profile, locking_profile, sash)")));
    let (len, qty) = rest.split_once('*').unwrap_or((rest, "1"));
    let length = parse_length(len, Some(Unit::Mm)).unwrap_or_else(|e| fail(&e.to_string()));
    let quantity = qty.trim().parse().unwrap_or_else(|_| fail(&format!("'{qty}' is not a quantity")));
    ExistingStock { profile, length_mm: length.mm(), quantity, confirmed: true }
}

fn print_aluminium(a: &AluminiumReport) {
    println!("ALUMINIUM REQUIREMENTS");
    println!("{:<16}{:>8}{:>16}{:>14}", "Profile", "Pieces", "Length (mm)", "Length (ft)");
    for p in &a.profiles {
        println!("{:<16}{:>8}{:>16}{:>14}", p.profile.label(), p.total_pieces, money(p.total_length_mm), money(p.total_length_ft));
    }
    println!(
        "{:<16}{:>8}{:>16}{:>14}\n",
        "Grand total",
        a.grand_total_pieces,
        money(a.grand_total_length_mm),
        money(a.grand_total_length_ft)
    );
}

fn print_rubber(r: &RubberReport) {
    println!("GLASS-PANEL RUBBER");
    println!("Total rubber:   {} m", r.total_metres.round_dp(4));
    println!("Rate per metre: KSh {}", money(r.rate_per_metre_ksh));
    println!("Rubber cost:    KSh {}\n", money(r.consumption_cost_ksh));
}

fn status(s: PlanStatus) -> &'static str {
    match s {
        PlanStatus::Verified => "verified",
        PlanStatus::Unconfirmed => "UNCONFIRMED",
    }
}

fn print_plan(title: &str, p: &StockPlan) {
    println!("{title} (cutting plan {})", status(p.status));
    println!("{:<16}{:>12}{:>14}{:>12}{:>10}", "Profile", "Bars to buy", "Lower bound", "Existing", "Status");
    for x in &p.profiles {
        println!(
            "{:<16}{:>12}{:>14}{:>12}{:>10}",
            x.profile.label(),
            x.new_bars_to_buy,
            x.lower_bound_bars_without_existing,
            x.existing_pieces_used,
            status(x.status)
        );
        for i in &x.issues {
            println!("    ! {i:?}");
        }
    }
    println!("Total bars: {}   Whole-bar cost: KSh {}\n", p.total_new_bars, money(p.full_bar_purchase_cost_ksh));
}

fn print_total(t: &ProjectTotal) {
    let range = |lo: Decimal, hi: Decimal| {
        if lo == hi { money(lo) } else { format!("{} - {}", money(lo), money(hi)) }
    };
    println!("  Aluminium {:>12}   Rubber {:>10}   Fittings {:>10}", money(t.aluminium_ksh), money(t.rubber_ksh), money(t.fittings_ksh));
    println!("  Labour    {:>12}   Glass sheets {:>6}   Transport {}", money(t.labour_ksh), money(t.glass_sheets_ksh), range(t.transport_low_ksh, t.transport_high_ksh));
    println!("  ESTIMATED TOTAL: KSh {} {}", range(t.total_low_ksh, t.total_high_ksh), if t.confirmed { "(confirmed)" } else { "(UNCONFIRMED)" });
    for n in &t.notes {
        println!("  note: {n}");
    }
}

fn print_full(c: &FullCosting) {
    println!("GLASS PANELS (actual cutting sizes, {} panels)", c.panels.total_panels);
    for g in &c.panels.groups {
        println!("  {} x {} mm  x {}", g.width_mm, g.height_mm, g.count);
    }
    println!();
    print_plan("STOCK OPTION A: buy new bars", &c.stock_option_a);
    if let (Some(b), Some(u)) = (&c.stock_option_b, &c.existing_stock_utilisation) {
        print_plan("STOCK OPTION B: use existing stock, buy the rest", b);
        println!("Existing stock used: {} pieces, {} mm of cuts\n", u.pieces_used, money(u.cut_length_mm));
    }
    println!("FITTINGS");
    for l in &c.fittings.lines {
        println!("  {:<18}{:>6} x {:>6} = KSh {}", l.kind.label(), l.total_quantity, money(l.unit_price_ksh), money(l.total_cost_ksh));
    }
    println!("\nLABOUR: {} small x {} + {} large x {} = KSh {}", c.labour.small_windows, c.labour.small_rate_ksh, c.labour.large_windows, c.labour.large_rate_ksh, money(c.labour.total_ksh));
    match &c.transport {
        TransportCost::Confirmed { amount_ksh } => println!("TRANSPORT: KSh {} (confirmed)", money(*amount_ksh)),
        TransportCost::Unconfirmed { min_ksh, max_ksh } => println!("TRANSPORT: KSh {min_ksh} - {max_ksh} (UNCONFIRMED)"),
    }
    if let Some(g) = &c.glass_sheets {
        println!("\nGLASS SHEETS ({:?})", g.status);
        for l in &g.lines {
            println!("  {}x{} ft  windows {}  sheets {:?}  [{:?}]", l.rounded_width_ft, l.rounded_height_ft, l.windows, l.sheets, l.status);
        }
        println!("  total sheets {:?}  cost {:?}", g.total_sheets, g.cost_ksh.map(money));
    }
    println!("\nPROJECT TOTALS");
    println!("Aluminium on consumption basis:");
    print_total(&c.totals_consumption);
    println!("Aluminium on whole-bar basis (Option A):");
    print_total(&c.totals_whole_bars);
    if let Some(t) = &c.totals_existing_plus_purchase {
        println!("Aluminium on existing stock + purchase basis (Option B):");
        print_total(t);
    }
}
