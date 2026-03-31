use std::sync::Arc;

use serde_json::json;
use wiremock::matchers::{method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crossref_lib::client::CrossrefClient;
use crossref_lib::config::Config;
use crossref_lib::models::SearchQueryBuilder;

/// Build a minimal config with a polite email for tests.
fn test_config() -> Arc<Config> {
    Arc::new(Config {
        email: Some("test@example.com".to_string()),
        proxy: None,
        default_rows: 10,
        cache_ttl_days: 0,
        cache_dir: None,
        default_format: None,
        fuzzy_finder_cmd: None,
    })
}

/// Minimal valid Unpaywall JSON response.
fn unpaywall_oa_response(doi: &str) -> serde_json::Value {
    json!({
        "doi": doi,
        "is_oa": true,
        "oa_status": "gold",
        "best_oa_location": {
            "url_for_pdf": "https://example.com/paper.pdf",
            "url": "https://example.com/paper"
        }
    })
}

fn unpaywall_closed_response(doi: &str) -> serde_json::Value {
    json!({
        "doi": doi,
        "is_oa": false,
        "oa_status": "closed",
        "best_oa_location": null
    })
}

/// Return a minimal but valid Work JSON object (used in both single and list responses).
fn work_json(doi: &str, title: &str, family: &str) -> serde_json::Value {
    let prefix = doi.split('/').next().unwrap_or("10.1234");
    json!({
        "publisher": "Test Publisher",
        "title": [title],
        "DOI": doi,
        "URL": format!("http://dx.doi.org/{}", doi),
        "type": "journal-article",
        "prefix": prefix,
        "member": "1234",
        "source": "Crossref",
        "references-count": 0,
        "is-referenced-by-count": 0,
        "indexed": {
            "date-parts": [[2021, 6, 1]],
            "date-time": "2021-06-01T00:00:00Z",
            "timestamp": 1622505600000_u64
        },
        "deposited": {
            "date-parts": [[2021, 6, 1]],
            "date-time": "2021-06-01T00:00:00Z",
            "timestamp": 1622505600000_u64
        },
        "issued": { "date-parts": [[2021, 6, 1]] },
        "author": [
            { "family": family, "given": "Test", "sequence": "first", "affiliation": [] }
        ],
        "container-title": ["Journal of Testing"],
        "volume": "10",
        "issue": "2",
        "page": "1-10",
        "content-domain": { "domain": [], "crossmark-restriction": false }
    })
}

/// Minimal valid Crossref single-work response envelope.
fn crossref_work_response(doi: &str) -> serde_json::Value {
    json!({
        "status": "ok",
        "message-type": "work",
        "message-version": "1.0.0",
        "message": work_json(doi, "A Test Article", "Smith")
    })
}

/// Minimal valid Crossref works-list response.
fn crossref_works_list_response(doi: &str) -> serde_json::Value {
    json!({
        "status": "ok",
        "message-type": "work-list",
        "message-version": "1.0.0",
        "message": {
            "total-results": 1,
            "items-per-page": 10,
            "query": { "start-index": 0, "search-terms": "test" },
            "facets": {},
            "next-cursor": null,
            "items": [
                work_json(doi, "A Search Result", "Doe")
            ]
        }
    })
}

// ─── fetch_unpaywall tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_fetch_unpaywall_oa() {
    let server = MockServer::start().await;
    let doi = "10.1234/test";

    Mock::given(method("GET"))
        .and(path(format!("/v2/{doi}")))
        .and(query_param("email", "test@example.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(unpaywall_oa_response(doi)))
        .mount(&server)
        .await;

    let cfg = test_config();
    let client =
        CrossrefClient::new_with_base_urls(cfg, None, Some(format!("{}/v2", server.uri())))
            .unwrap();

    let record = client.fetch_unpaywall(doi).await.unwrap();
    assert!(record.is_oa);
    assert_eq!(record.oa_status, "gold");
    assert_eq!(
        record.best_oa_location.unwrap().url_for_pdf.unwrap(),
        "https://example.com/paper.pdf"
    );
}

#[tokio::test]
async fn test_fetch_unpaywall_closed() {
    let server = MockServer::start().await;
    let doi = "10.1234/closed";

    Mock::given(method("GET"))
        .and(path(format!("/v2/{doi}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(unpaywall_closed_response(doi)))
        .mount(&server)
        .await;

    let cfg = test_config();
    let client =
        CrossrefClient::new_with_base_urls(cfg, None, Some(format!("{}/v2", server.uri())))
            .unwrap();

    let record = client.fetch_unpaywall(doi).await.unwrap();
    assert!(!record.is_oa);
    assert_eq!(record.oa_status, "closed");
    assert!(record.best_oa_location.is_none());
}

// ─── fetch_work Unpaywall enrichment tests ───────────────────────────────────

#[tokio::test]
async fn test_fetch_work_enriched_with_unpaywall() {
    let crossref_server = MockServer::start().await;
    let unpaywall_server = MockServer::start().await;
    let doi = "10.1234/enriched";

    // The crossref crate builds the path as /works/{doi} with raw slashes
    Mock::given(method("GET"))
        .and(path(format!("/works/{doi}")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(crossref_work_response(doi)),
        )
        .mount(&crossref_server)
        .await;

    Mock::given(method("GET"))
        .and(path(format!("/v2/{doi}")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(unpaywall_oa_response(doi)),
        )
        .mount(&unpaywall_server)
        .await;

    let cfg = test_config();
    let client = CrossrefClient::new_with_base_urls(
        cfg,
        Some(crossref_server.uri()),
        Some(format!("{}/v2", unpaywall_server.uri())),
    )
    .unwrap();

    let work = client.fetch_work(doi).await.unwrap();
    assert_eq!(work.is_oa, Some(true));
    assert_eq!(work.oa_status.as_deref(), Some("gold"));
    assert!(work.pdf_url.is_some());
}

#[tokio::test]
async fn test_fetch_work_unpaywall_failure_is_non_fatal() {
    let crossref_server = MockServer::start().await;
    let unpaywall_server = MockServer::start().await;
    let doi = "10.1234/nounpaywall";

    Mock::given(method("GET"))
        .and(path(format!("/works/{doi}")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(crossref_work_response(doi)),
        )
        .mount(&crossref_server)
        .await;

    // Unpaywall returns 404
    Mock::given(method("GET"))
        .and(path(format!("/v2/{doi}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&unpaywall_server)
        .await;

    let cfg = test_config();
    let client = CrossrefClient::new_with_base_urls(
        cfg,
        Some(crossref_server.uri()),
        Some(format!("{}/v2", unpaywall_server.uri())),
    )
    .unwrap();

    // fetch_work should succeed even when Unpaywall fails
    let work = client.fetch_work(doi).await.unwrap();
    assert_eq!(work.doi, doi);
    assert!(work.is_oa.is_none(), "OA fields should remain None on Unpaywall failure");
}

// ─── search tests ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_search_returns_results() {
    let server = MockServer::start().await;
    let doi = "10.1234/search";

    Mock::given(method("GET"))
        .and(path_regex(r"^/works"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(crossref_works_list_response(doi)),
        )
        .mount(&server)
        .await;

    let cfg = test_config();
    let client =
        CrossrefClient::new_with_base_urls(cfg, Some(server.uri()), None).unwrap();

    let query = SearchQueryBuilder::default()
        .query(Some("test query".to_string()))
        .rows(10u32)
        .build()
        .unwrap();

    let result = client.search(&query).await.unwrap();
    assert_eq!(result.total_results, 1);
    assert_eq!(result.items.len(), 1);
    assert_eq!(result.items[0].doi, doi);
}

#[tokio::test]
async fn test_search_with_filters() {
    let server = MockServer::start().await;
    let doi = "10.1234/filtered";

    Mock::given(method("GET"))
        .and(path_regex(r"^/works"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(crossref_works_list_response(doi)),
        )
        .mount(&server)
        .await;

    let cfg = test_config();
    let client =
        CrossrefClient::new_with_base_urls(cfg, Some(server.uri()), None).unwrap();

    let query = SearchQueryBuilder::default()
        .query(Some("machine learning".to_string()))
        .year_from(Some(2020))
        .year_to(Some(2023))
        .work_type(Some("journal-article".to_string()))
        .open_access(true)
        .rows(5u32)
        .build()
        .unwrap();

    let result = client.search(&query).await.unwrap();
    assert_eq!(result.items.len(), 1);
}

// ─── download_pdf tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_download_pdf_success() {
    let unpaywall_server = MockServer::start().await;
    let pdf_server = MockServer::start().await;
    let doi = "10.1234/pdf";
    let pdf_bytes = b"%PDF-1.4 fake pdf content";

    let pdf_url = format!("{}/paper.pdf", pdf_server.uri());
    let unpaywall_resp = json!({
        "doi": doi,
        "is_oa": true,
        "oa_status": "gold",
        "best_oa_location": {
            "url_for_pdf": pdf_url,
            "url": pdf_url.clone()
        }
    });

    Mock::given(method("GET"))
        .and(path(format!("/v2/{doi}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(unpaywall_resp))
        .mount(&unpaywall_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/paper.pdf"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(pdf_bytes.to_vec())
                .insert_header("content-type", "application/pdf"),
        )
        .mount(&pdf_server)
        .await;

    let cfg = test_config();
    let client =
        CrossrefClient::new_with_base_urls(cfg, None, Some(format!("{}/v2", unpaywall_server.uri())))
            .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let dest = client.download_pdf(doi, dir.path()).await.unwrap();

    assert!(dest.exists(), "downloaded PDF file should exist");
    assert_eq!(std::fs::read(&dest).unwrap(), pdf_bytes);
}

#[tokio::test]
async fn test_download_pdf_no_oa_returns_error() {
    let unpaywall_server = MockServer::start().await;
    let doi = "10.1234/nopdf";

    Mock::given(method("GET"))
        .and(path(format!("/v2/{doi}")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(unpaywall_closed_response(doi)),
        )
        .mount(&unpaywall_server)
        .await;

    let cfg = test_config();
    let client =
        CrossrefClient::new_with_base_urls(cfg, None, Some(format!("{}/v2", unpaywall_server.uri())))
            .unwrap();

    let dir = tempfile::tempdir().unwrap();
    let result = client.download_pdf(doi, dir.path()).await;

    assert!(result.is_err(), "should fail when no OA PDF is available");
}
