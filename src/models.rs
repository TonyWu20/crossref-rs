use std::collections::BTreeMap;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

/// Normalised metadata for a single publication, populated from the Crossref
/// `Work` response and optionally enriched with Unpaywall OA data.
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(setter(into), default)]
pub struct WorkMeta {
    pub doi: String,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    /// Journal / book title (first element of `container-title`)
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    /// Page range, e.g. "123-145"
    pub pages: Option<String>,
    pub publisher: Option<String>,
    /// Crossref type string, e.g. "journal-article"
    pub work_type: Option<String>,
    // Unpaywall OA fields
    pub is_oa: Option<bool>,
    pub oa_status: Option<String>,
    pub pdf_url: Option<String>,
}

impl Default for WorkMeta {
    fn default() -> Self {
        Self {
            doi: String::new(),
            title: None,
            authors: Vec::new(),
            year: None,
            journal: None,
            volume: None,
            issue: None,
            pages: None,
            publisher: None,
            work_type: None,
            is_oa: None,
            oa_status: None,
            pdf_url: None,
        }
    }
}

/// A single BibTeX entry in a format compatible with `serde_bibtex` serialization.
/// Fields use `BTreeMap` so the output is deterministically ordered.
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(setter(into), default)]
pub struct BibRecord {
    pub entry_type: String,
    pub entry_key: String,
    pub fields: BTreeMap<String, String>,
}

impl Default for BibRecord {
    fn default() -> Self {
        Self {
            entry_type: String::new(),
            entry_key: String::new(),
            fields: BTreeMap::new(),
        }
    }
}

/// Parameters for a Crossref works search request.
#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(setter(into), default)]
pub struct SearchQuery {
    pub query: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub year_from: Option<i32>,
    pub year_to: Option<i32>,
    /// Crossref type filter, e.g. "journal-article"
    pub work_type: Option<String>,
    pub open_access: bool,
    pub rows: u32,
    pub sort: Option<String>,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: None,
            title: None,
            author: None,
            year_from: None,
            year_to: None,
            work_type: None,
            open_access: false,
            rows: 10,
            sort: None,
        }
    }
}

/// Paged result set returned by a search request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub items: Vec<WorkMeta>,
    pub total_results: u64,
}
