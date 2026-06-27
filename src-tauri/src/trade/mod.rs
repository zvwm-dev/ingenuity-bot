//! Trade API integration: rate-limited, ToS-compliant access to the PoE2 trade
//! endpoints, plus typed models for the parts we use.

pub mod client;
pub mod error;
pub mod models;
pub mod rate_limit;

pub use client::{TradeClient, USER_AGENT};
pub use error::{Result, TradeError};
