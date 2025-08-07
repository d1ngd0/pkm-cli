use thiserror::Error;

// Result is a convienince type for T, pkm::Error
pub type Result<T> = std::result::Result<T, Error>;

// Error is the main error
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    InvalidZettelID(String),

    #[error("command line parsing error: {0}")]
    CommandError(#[from] clap::Error),

    #[error("indexing error: {0}")]
    IndexError(#[from] tantivy::TantivyError),

    #[error("IO Error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("Templating Error: {0}")]
    TemplatingError(#[from] tera::Error),

    #[error("Markdown Parsing Error: {0:?}")]
    MarkdownParserError(markdown::message::Message),

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
