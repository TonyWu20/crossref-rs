//! # crossref-rs
//!
//! A high-performance Crossref literature metadata and BibTeX management library.
//! Supports both a standard CLI binary and a Nushell native plugin.

pub mod bibtex;
pub mod cache;
pub mod client;
pub mod config;
pub mod error;
pub mod models;
pub mod utils;

// Convenience re-exports for downstream crates and binaries
pub use client::CrossrefClient;
pub use config::Config;
pub use error::{CrossrefError, Result};
pub use models::{BibRecord, SearchQuery, SearchResult, WorkMeta};
pub use utils::KeyStyle;
