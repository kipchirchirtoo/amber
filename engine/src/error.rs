use thiserror::Error;

/// Every way the engine can refuse an input. The engine never guesses:
/// anything ambiguous or physically impossible becomes an error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EngineError {
    #[error("'{0}' is not a valid number")]
    InvalidNumber(String),
    #[error("unknown unit '{0}' (use mm, ft or m)")]
    UnknownUnit(String),
    #[error("no unit given for '{0}' and no default unit set")]
    MissingUnit(String),
    #[error("could not read '{0}' as a size; expected WIDTH x HEIGHT, e.g. '4 x 5 ft'")]
    MalformedSize(String),
    #[error("{what} must be greater than zero")]
    NonPositive { what: String },
    #[error("{what} exceeds the sanity limit of {limit_mm} mm")]
    TooLarge { what: String, limit_mm: u32 },
    #[error("quantity must be at least 1 (window '{0}')")]
    ZeroQuantity(String),
    #[error("{what} exceeds the limit of {limit}")]
    LimitExceeded { what: String, limit: u64 },
    #[error("saw kerf must be 0 or more and shorter than the stock bar")]
    InvalidKerf,
    #[error("window '{window}': glass panel would be {panel_width_mm} x {panel_height_mm} mm, which is not possible")]
    PanelNotPositive { window: String, panel_width_mm: String, panel_height_mm: String },
    #[error("invalid rules: {0}")]
    InvalidRules(String),
    #[error("amount must be zero or more")]
    NegativeAmount,
    #[error("the order has no windows")]
    EmptyOrder,
    #[error(
        "window '{window}' ({width_mm} x {height_mm} mm) is too small: {profile} would have a cut length of {cut_mm} mm"
    )]
    CutNotPositive {
        window: String,
        profile: String,
        width_mm: String,
        height_mm: String,
        cut_mm: String,
    },
}

pub type Result<T> = std::result::Result<T, EngineError>;
