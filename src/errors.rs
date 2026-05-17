use thiserror::Error;

pub type Result<T> = std::result::Result<T, BlazePolyError>;

#[derive(Debug, Error)]
pub enum BlazePolyError {
    /// Anything caused by external systems (RPC, API, network)
    #[error("external error: {0}")]
    External(String),

    /// Bad inputs, config, or precondition failures
    #[error("invalid input: {0}")]
    Invalid(String),

    /// Internal bugs / invariants broken
    #[error("internal error: {0}")]
    Internal(String),
}
