use std::{num::ParseIntError, str::Utf8Error};

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

    #[error("LSP Error: {0}")]
    LSPError(String),

    #[error("Could not parse int: {0}")]
    ParseIntError(#[from] ParseIntError),

    #[error("UTF8 Error {0}")]
    UTF8Error(#[from] Utf8Error),

    #[error("Send Error: {0}")]
    SendError(#[from] tokio::sync::mpsc::error::SendError<super::Response>),
    #[error("Recieve Error: {0}")]
    RecieveError(#[from] tokio::sync::mpsc::error::TryRecvError),
}
