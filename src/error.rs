use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrossrefError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Crossref API error: {0}")]
    Api(String),

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
