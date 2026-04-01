use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrossrefError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Crossref API error: {0}")]
    Api(String),

    /// The Crossref API returned a response our code could not decode.
    /// Typically caused by missing or unexpected fields in unusual entry types
    /// (datasets, book chapters, components) that the API returned alongside
    /// standard journal articles.
    #[error("Failed to decode Crossref API response: {0}")]
    Parse(String),

    /// Invalid user input — the error message contains actionable guidance.
    #[error("{0}")]
    Usage(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Cache error: {0}")]
    Cache(String),

    #[error("BibTeX error: {0}")]
    Bibtex(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Unpaywall error: {0}")]
    Unpaywall(String),

    #[error("PDF download error: {0}")]
    PdfDownload(String),

    #[error("Builder error: {0}")]
    Builder(String),
}

pub type Result<T> = std::result::Result<T, CrossrefError>;
