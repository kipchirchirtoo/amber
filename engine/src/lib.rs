//! amber_engine: the only place in the Amber Stores system where arithmetic
//! happens. No AI, UI or database code may compute a business number.

pub mod aluminium;
pub mod costing;
pub mod error;
pub mod fittings;
pub mod glass;
pub mod glass_sheets;
pub mod labour;
pub mod optimiser;
pub mod order;
pub mod rubber;
pub mod rules;
pub mod transport;
pub mod units;

pub use aluminium::{calculate_aluminium, round_display, AluminiumReport, CutGroup, ProfileRequirement};
pub use optimiser::{
    optimise_stock, plan_profile, verify_profile_plan, Bin, BinSource, ExistingStock, PlanIssue,
    PlanStatus, ProfilePlan, StockConfig, StockPlan,
};
pub use error::{EngineError, Result};
pub use order::{Order, WindowSpec};
pub use rules::{Basis, CutRule, Profile, Rules};
pub use units::{parse_length, parse_size, Length, Size, Unit, MM_PER_FT, MM_PER_M};
pub use costing::{
    calculate_full_costing, default_output, AluminiumBasis, CostingOptions, DefaultOutput, FullCosting,
    ProjectTotal,
};
pub use fittings::{calculate_fittings, FittingsReport};
pub use glass::{calculate_panels, PanelGroup, PanelReport};
pub use glass_sheets::{calculate_glass_sheets, GlassSheetReport, GlassStatus, LineStatus};
pub use labour::{calculate_labour, LabourReport};
pub use rubber::{calculate_rubber, calculate_rubber_rolls, full_roll_purchase, RubberReport, RubberRollReport};
pub use rules::FittingKind;
pub use transport::{calculate_transport, TransportCost};
