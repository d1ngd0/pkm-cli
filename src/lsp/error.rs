use thiserror::Error;

// Result is a convienince type for T, pkm::Error
pub type Result<T> = std::result::Result<T, Error>;

// Error is the main error
#[derive(Debug, Error)]
pub enum Error {
    #[error("response not yet ready")]
    NotReady,

    #[error("serialization Error {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),
}
