use thiserror::Error;

// Result is a convienince type for T, pkm::Error
pub type Result<T> = std::result::Result<T, Error>;

// Error is the main error
#[derive(Debug, Error)]
pub enum Error {
    #[error("command line parsing error")]
    CommandError(#[from] clap::Error),

    #[error("indexing error")]
    IndexError(#[from] tantivy::TantivyError),

    #[error("IO Error")]
    IOError(#[from] std::io::Error),

    #[error("Templating Error")]
    TemplatingError(#[from] tera::Error),

    #[error("unknown data store error")]
    Unknown,
}
