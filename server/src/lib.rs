//! amber_server: HTTP API over the Amber Stores calculation engine.
//! This crate stores and serves results; it never calculates anything itself.

pub mod api;
pub mod assistant;
pub mod auth;
pub mod calc;
pub mod error;
pub mod quote;
pub mod store;

pub use api::{router, AppState};
