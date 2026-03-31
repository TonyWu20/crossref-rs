use std::sync::Arc;

use crate::config::Config;
use crate::error::{CrossrefError, Result};
use crate::models::{SearchQuery, SearchResult, WorkMeta};
use crate::utils::normalise_doi;

/// Unpaywall OA record for a single DOI.
#[derive(Debug, serde::Deserialize)]
pub struct UnpaywallRecord {
    pub is_oa: bool,
    pub oa_status: String,
    /// Best available open-access PDF URL, if any.
    pub best_oa_location: Option<UnpaywallLocation>,
}

#[derive(Debug, serde::Deserialize)]
pub struct UnpaywallLocation {
    pub url_for_pdf: Option<String>,
}

/// High-level API client.
///
/// Wraps the synchronous [`crossref::Crossref`] client (created fresh per
/// blocking call) and a shared `reqwest::Client` for async Unpaywall queries.
pub struct CrossrefClient {
    config: Arc<Config>,
    http: reqwest::Client,
}

impl CrossrefClient {
    /// Construct a new client from the resolved configuration.
    pub fn new(config: Arc<Config>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(build_user_agent(&config))
            .build()?;
        Ok(Self { config, http })
    }

    // ─── Crossref API ───────────────────────────────────────────────────────

    /// Fetch metadata for a single DOI.
    pub async fn fetch_work(&self, doi: &str) -> Result<WorkMeta> {
        let doi = normalise_doi(doi);
        let email = self.config.email.clone();

        // `crossref::Crossref` is !Send (uses Rc<Client>), so build it inside
        // the blocking thread each time.
        let work = tokio::task::spawn_blocking(move || {
            let mut builder = crossref::Crossref::builder();
            if let Some(ref e) = email {
                builder = builder.polite(e.as_str());
            }
            let client = builder
                .build()
                .map_err(|e| CrossrefError::Api(e.to_string()))?;
            client
                .work(&doi)
                .map_err(|e| CrossrefError::Api(e.to_string()))
        })
        .await
        .map_err(|e| CrossrefError::Api(e.to_string()))??;

        Ok(map_work(work))
    }

    /// Fetch metadata for multiple DOIs, returning results in order.
    pub async fn fetch_works(&self, dois: &[&str]) -> Vec<Result<WorkMeta>> {
        let mut results = Vec::with_capacity(dois.len());
        for doi in dois {
            results.push(self.fetch_work(doi).await);
        }
        results
    }

    /// Execute a search query and return a page of results.
    pub async fn search(&self, query: &SearchQuery) -> Result<SearchResult> {
        let query = query.clone();
        let email = self.config.email.clone();

        let work_list = tokio::task::spawn_blocking(move || {
            let mut builder = crossref::Crossref::builder();
            if let Some(ref e) = email {
                builder = builder.polite(e.as_str());
            }
            let client = builder
                .build()
                .map_err(|e| CrossrefError::Api(e.to_string()))?;

            let wq = build_works_query(&query);
            let result = client
                .works(wq)
                .map_err(|e| CrossrefError::Api(e.to_string()))?;
            Ok::<_, CrossrefError>(result)
        })
        .await
        .map_err(|e| CrossrefError::Api(e.to_string()))??;

        let total_results = work_list.total_results as u64;
        let items = work_list.items.into_iter().map(map_work).collect();
        Ok(SearchResult { items, total_results })
    }

    // ─── Unpaywall API ──────────────────────────────────────────────────────

    /// Query Unpaywall for OA information about a DOI.
    pub async fn fetch_unpaywall(&self, doi: &str) -> Result<UnpaywallRecord> {
        let doi = normalise_doi(doi);
        let email = self
            .config
            .email
            .as_deref()
            .unwrap_or("anonymous@example.com")
            .to_string();
        let url = format!(
            "https://api.unpaywall.org/v2/{}?email={}",
            doi, email
        );
        let record: UnpaywallRecord = self
            .http
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .map_err(|e| CrossrefError::Unpaywall(e.to_string()))?;
        Ok(record)
    }

    /// Download the best OA PDF to `dest_dir` / `<DOI>.pdf`.
    /// Returns the path where the file was written.
    pub async fn download_pdf(
        &self,
        doi: &str,
        dest_dir: &std::path::Path,
    ) -> Result<std::path::PathBuf> {
        let record = self.fetch_unpaywall(doi).await?;
        let pdf_url = record
            .best_oa_location
            .and_then(|loc| loc.url_for_pdf)
            .ok_or_else(|| CrossrefError::PdfDownload(format!("no OA PDF found for {}", doi)))?;

        let bytes = self
            .http
            .get(&pdf_url)
            .send()
            .await?
            .bytes()
            .await
            .map_err(|e| CrossrefError::PdfDownload(e.to_string()))?;

        let safe_doi = normalise_doi(doi).replace('/', "_");
        let dest = dest_dir.join(format!("{}.pdf", safe_doi));
        std::fs::write(&dest, &bytes)?;
        Ok(dest)
    }
}

// ─── Helper functions ────────────────────────────────────────────────────────

/// Build an appropriate `User-Agent` string.
fn build_user_agent(config: &Config) -> String {
    let version = env!("CARGO_PKG_VERSION");
    match &config.email {
        Some(email) => format!("crossref-rs/{} (mailto:{})", version, email),
        None => format!("crossref-rs/{}", version),
    }
}

/// Map a `crossref::Work` into our `WorkMeta` model.
fn map_work(w: crossref::Work) -> WorkMeta {
    let title = w.title.into_iter().next();

    let authors: Vec<String> = w
        .author
        .unwrap_or_default()
        .into_iter()
        .map(|c| match c.given {
            Some(given) => format!("{}, {}", c.family, given),
            None => c.family,
        })
        .collect();

    // DateField is not in crossref's public API; extract year directly from the
    // raw date-parts nested array: [[year, month?, day?], …]
    let year = w
        .issued
        .date_parts
        .0
        .first()
        .and_then(|parts| parts.first())
        .and_then(|opt_y| *opt_y)
        .map(|y| y as i32);

    let journal = w
        .container_title
        .and_then(|v| v.into_iter().next());

    WorkMeta {
        doi: w.doi,
        title,
        authors,
        year,
        journal,
        volume: w.volume,
        issue: w.issue,
        pages: w.page,
        publisher: Some(w.publisher),
        work_type: Some(w.type_),
        is_oa: None,
        oa_status: None,
        pdf_url: None,
    }
}

/// Translate our `SearchQuery` into a `crossref::WorksQuery`.
fn build_works_query(q: &SearchQuery) -> crossref::WorksQuery {
    let term = q
        .query
        .as_deref()
        .unwrap_or("")
        .to_string();
    let mut wq = crossref::WorksQuery::new(term);

    if let Some(ref author) = q.author {
        wq = wq.field_query(crossref::FieldQuery::author(author.as_str()));
    }
    if let Some(ref title) = q.title {
        wq = wq.field_query(crossref::FieldQuery::title(title.as_str()));
    }

    wq = wq.result_control(crossref::WorkResultControl::Standard(
        crossref::query::ResultControl::Rows(q.rows as usize),
    ));

    if let Some(ref sort) = q.sort {
        let sort_val = match sort.as_str() {
            "score" => crossref::Sort::Score,
            "updated" => crossref::Sort::Updated,
            "deposited" => crossref::Sort::Deposited,
            "indexed" => crossref::Sort::Indexed,
            "published" => crossref::Sort::Published,
            _ => crossref::Sort::Score,
        };
        wq = wq.sort(sort_val);
    }

    wq
}
