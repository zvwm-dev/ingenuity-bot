use thiserror::Error;

/// Errors that can occur talking to the PoE2 trade API.
#[derive(Debug, Error)]
pub enum TradeError {
    #[error("network error: {0}")]
    Http(#[from] reqwest::Error),

    /// The API told us we are rate limited. Carries the number of seconds to wait
    /// (from the `Retry-After` header) before trying again.
    #[error("rate limited by the trade API; retry in {0}s")]
    RateLimited(u64),

    /// The API returned a non-success HTTP status.
    #[error("trade API returned HTTP {status}: {body}")]
    Status { status: u16, body: String },

    /// We could not parse the API response into the expected shape.
    #[error("could not parse trade API response: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, TradeError>;
