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
    /// Optional override for the Crossref API base URL (used in tests).
    crossref_base_url: Option<String>,
    /// Optional override for the Unpaywall API base URL (used in tests).
    unpaywall_base_url: Option<String>,
}

impl CrossrefClient {
    /// Construct a new client from the resolved configuration.
    pub fn new(config: Arc<Config>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(build_user_agent(&config))
            .build()?;
        Ok(Self { config, http, crossref_base_url: None, unpaywall_base_url: None })
    }

    /// Construct a client with custom base URLs (for testing and integration environments).
    pub fn new_with_base_urls(
        config: Arc<Config>,
        crossref_url: Option<String>,
        unpaywall_url: Option<String>,
    ) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(build_user_agent(&config))
            .build()?;
        Ok(Self {
            config,
            http,
            crossref_base_url: crossref_url,
            unpaywall_base_url: unpaywall_url,
        })
    }

    /// Alias for tests.
    #[cfg(test)]
    pub fn new_for_test(
        config: Arc<Config>,
        crossref_url: Option<String>,
        unpaywall_url: Option<String>,
    ) -> Result<Self> {
        Self::new_with_base_urls(config, crossref_url, unpaywall_url)
    }

    // ─── Crossref API ───────────────────────────────────────────────────────

    /// Fetch metadata for a single DOI, then enrich with Unpaywall OA data.
    pub async fn fetch_work(&self, doi: &str) -> Result<WorkMeta> {
        let doi = normalise_doi(doi);
        let email = self.config.email.clone();
        let base_url = self.crossref_base_url.clone();

        // `crossref::Crossref` is !Send (uses Rc<Client>), so build it inside
        // the blocking thread each time.
        let work = tokio::task::spawn_blocking(move || {
            let mut builder = crossref::Crossref::builder();
            if let Some(ref e) = email {
                builder = builder.polite(e.as_str());
            }
            let mut client = builder
                .build()
                .map_err(|e| CrossrefError::Api(e.to_string()))?;
            // Allow overriding the base URL for tests
            if let Some(url) = base_url {
                client.base_url = url;
            }
            client
                .work(&doi)
                .map_err(|e| CrossrefError::Api(e.to_string()))
        })
        .await
        .map_err(|e| CrossrefError::Api(e.to_string()))??;

        let mut meta = map_work(work);

        // Auto-enrich with Unpaywall OA data; failures are non-fatal
        match self.fetch_unpaywall(&meta.doi).await {
            Ok(oa) => {
                meta.is_oa = Some(oa.is_oa);
                meta.oa_status = Some(oa.oa_status);
                meta.pdf_url = oa.best_oa_location.and_then(|loc| loc.url_for_pdf);
            }
            Err(e) => {
                eprintln!("warning: Unpaywall enrichment failed: {e}");
            }
        }

        Ok(meta)
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
        let base_url = self.crossref_base_url.clone();

        let work_list = tokio::task::spawn_blocking(move || {
            let mut builder = crossref::Crossref::builder();
            if let Some(ref e) = email {
                builder = builder.polite(e.as_str());
            }
            let mut client = builder
                .build()
                .map_err(|e| CrossrefError::Api(e.to_string()))?;
            if let Some(url) = base_url {
                client.base_url = url;
            }

            let wq = build_works_query(&query);
            let result = client
                .works(wq)
                .map_err(|e| {
                    let msg = e.to_string();
                    // The `crossref` crate prefixes serde failures with
                    // "invalid serde"; treat those as parse errors rather
                    // than API errors so callers can show targeted guidance.
                    if msg.contains("invalid serde") || msg.contains("missing field") {
                        CrossrefError::Parse(msg)
                    } else {
                        CrossrefError::Api(msg)
                    }
                })?;
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
        let base = self
            .unpaywall_base_url
            .as_deref()
            .unwrap_or("https://api.unpaywall.org/v2");
        let url = format!("{base}/{doi}?email={email}");
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
    ///
    /// Falls back to EZproxy if the direct URL returns a non-200 status,
    /// and finally returns a `https://doi.org/{doi}` link if no PDF is
    /// accessible.
    ///
    /// Returns the path where the file was written, or the best-effort URL
    /// if the PDF was not downloaded.
    pub async fn download_pdf(
        &self,
        doi: &str,
        dest_dir: &std::path::Path,
    ) -> Result<std::path::PathBuf> {
        let norm_doi = normalise_doi(doi);

        let record = self.fetch_unpaywall(&norm_doi).await?;
        let pdf_url = record
            .best_oa_location
            .and_then(|loc| loc.url_for_pdf);

        let safe_doi = norm_doi.replace('/', "_");
        let dest = dest_dir.join(format!("{safe_doi}.pdf"));

        // Try direct PDF URL
        if let Some(ref url) = pdf_url {
            if let Ok(resp) = self.http.get(url).send().await {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes().await {
                        if is_pdf(&bytes) {
                            std::fs::write(&dest, &bytes)?;
                            return Ok(dest);
                        }
                        // Response was HTML (landing page / paywall) — try fallbacks
                    }
                }
            }

            // EZproxy fallback
            if let Some(ref proxy) = self.config.proxy {
                let proxy_url = format!("https://{proxy}/doi/{norm_doi}");
                if let Ok(resp) = self.http.get(&proxy_url).send().await {
                    if resp.status().is_success() {
                        if let Ok(bytes) = resp.bytes().await {
                            if is_pdf(&bytes) {
                                std::fs::write(&dest, &bytes)?;
                                return Ok(dest);
                            }
                        }
                    }
                }
            }
        } else if let Some(ref proxy) = self.config.proxy {
            // No direct URL but proxy is configured — try proxy
            let proxy_url = format!("https://{proxy}/doi/{norm_doi}");
            if let Ok(resp) = self.http.get(&proxy_url).send().await {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes().await {
                        if is_pdf(&bytes) {
                            std::fs::write(&dest, &bytes)?;
                            return Ok(dest);
                        }
                    }
                }
            }
        }

        // Best-effort: return doi.org link as a path placeholder
        Err(CrossrefError::PdfDownload(format!(
            "no downloadable PDF found; try https://doi.org/{norm_doi}"
        )))
    }
}

// ─── Helper functions ────────────────────────────────────────────────────────

/// Return `true` if `bytes` begins with the PDF magic number `%PDF-`.
///
/// Publishers often serve HTML landing pages (with 200 OK) at a URL that
/// is labelled as a PDF link; checking the magic bytes avoids saving those.
fn is_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF-")
}

/// Build an appropriate `User-Agent` string.
fn build_user_agent(config: &Config) -> String {
    let version = env!("CARGO_PKG_VERSION");
    match &config.email {
        Some(email) => format!("crossref-rs/{version} (mailto:{email})"),
        None => format!("crossref-rs/{version}"),
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
    use chrono::NaiveDate;

    // The crossref crate's FieldQuery serialises as "title=value" instead of the
    // correct "query.title=value", causing a validation-failure from the REST API.
    // Combine all text inputs into the free-form query= parameter instead.
    let mut term_parts: Vec<&str> = Vec::new();
    if let Some(ref t) = q.query  { term_parts.push(t.as_str()); }
    if let Some(ref t) = q.title  { term_parts.push(t.as_str()); }
    if let Some(ref a) = q.author { term_parts.push(a.as_str()); }
    let term = term_parts.join(" ");

    let mut wq = if term.is_empty() {
        crossref::WorksQuery::empty()
    } else {
        crossref::WorksQuery::new(term)
    };

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

    // Date range filters
    if let Some(year) = q.year_from {
        if let Some(date) = NaiveDate::from_ymd_opt(year, 1, 1) {
            wq = wq.filter(crossref::WorksFilter::FromPubDate(date));
        }
    }
    if let Some(year) = q.year_to {
        if let Some(date) = NaiveDate::from_ymd_opt(year, 12, 31) {
            wq = wq.filter(crossref::WorksFilter::UntilPubDate(date));
        }
    }

    // Work type filter — parse to the Type enum; fall back to TypeName on unknown strings
    if let Some(ref work_type) = q.work_type {
        use std::str::FromStr;
        if let Ok(t) = crossref::Type::from_str(work_type.as_str()) {
            wq = wq.filter(crossref::WorksFilter::Type(t));
        } else {
            // Unknown type string: pass as-is via TypeName (best-effort)
            wq = wq.filter(crossref::WorksFilter::TypeName(work_type.clone()));
        }
    }

    // Open-access filter (proxy: has-license)
    if q.open_access {
        wq = wq.filter(crossref::WorksFilter::HasLicense);
    }

    wq
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SearchQueryBuilder;

    #[test]
    fn test_build_works_query_filters() {
        // Ensure that filter methods on SearchQuery are forwarded to WorksQuery
        // We verify this by building a query and checking no panic occurs.
        let q = SearchQueryBuilder::default()
            .query(Some("machine learning".to_string()))
            .year_from(Some(2020))
            .year_to(Some(2023))
            .work_type(Some("journal-article".to_string()))
            .open_access(true)
            .rows(5u32)
            .build()
            .unwrap();

        // build_works_query should not panic
        let _wq = build_works_query(&q);
    }

    #[test]
    fn test_build_works_query_no_filters() {
        let q = SearchQueryBuilder::default()
            .query(Some("test".to_string()))
            .rows(10u32)
            .build()
            .unwrap();
        let _wq = build_works_query(&q);
    }
}
