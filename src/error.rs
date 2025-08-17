use std::sync::Arc;

use skim::SkimItem;
use thiserror::Error;

// Result is a convienince type for T, pkm::Error
pub type Result<T> = std::result::Result<T, Error>;

// Error is the main error
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    InvalidZettelID(String),

    #[error("Not Found: {0}")]
    NotFound(String),

    #[error("prompt error: {0}")]
    PromptError(#[from] inquire::error::InquireError),

    #[error("command line parsing error: {0}")]
    CommandError(#[from] clap::Error),

    #[error("indexing error: {0}")]
    IndexError(#[from] tantivy::TantivyError),

    #[error("indexing error: {0}")]
    OpenDirectoryError(#[from] tantivy::directory::error::OpenDirectoryError),

    #[error("indexing error: {0}")]
    QueryError(#[from] tantivy::query::QueryParserError),

    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Sending Error {0}")]
    ChannelSendError(#[from] crossbeam_channel::SendError<Arc<dyn SkimItem>>),

    #[error("Receiver Error {0}")]
    ChannelReceiverError(#[from] crossbeam_channel::RecvError),

    #[error("Templating Error: {0}")]
    TemplatingError(#[from] tera::Error),

    #[error("Markdown Parsing Error: {0:?}")]
    MarkdownParserError(markdown::message::Message),

    #[error("LSP Error: {0}")]
    LSPError(#[from] crate::lsp::Error),

    #[error("Serialization Error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("unknown data store error")]
    Unknown,
}

// For some reason this one didn't work with the #[from] so I
// had to manually make it. Whatever
impl From<markdown::message::Message> for Error {
    fn from(value: markdown::message::Message) -> Self {
        Self::MarkdownParserError(value)
    }
}
