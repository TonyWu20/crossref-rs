# Testing patterns

## Integration tests (tests/)

- HTTP mocking via `wiremock` (not `mockito`). Use `MockServer::start().await`.
- Crossref single-work mock: `path(format!("/works/{doi}"))` — raw DOI, **no URL encoding**.
- Crossref search mock: `path_regex(r"^/works")` (search uses query params, not path segments).
- A minimal but valid `Work` JSON must include: `publisher`, `title`, `DOI`, `URL`, `type`, `prefix`, `member`, `source`, `references-count`, `is-referenced-by-count`, `indexed` (with `date-parts`, `date-time`, `timestamp`), `deposited` (same), `issued`, `author` (array with `family`, `given`, `sequence`, `affiliation`), `container-title`, `content-domain`.
- Works-list JSON requires `query: { "start-index": 0, "search-terms": "..." }` in the `message` object.
- Tests that touch env vars must serialize on `static ENV_MUTEX: Mutex<()>` to avoid races.

## Unit tests (`#[cfg(test)]` in source files)

- Prefer `tempfile::NamedTempFile` / `tempfile::tempdir()` for any filesystem operations.
- `WorkMeta::default()` is available — use `..WorkMeta::default()` struct update syntax in fixtures.

## Key invariants to preserve

- `CrossrefClient::new_with_base_urls` must remain `pub` (not `#[cfg(test)]`) — integration tests in `tests/` are separate crates and cannot see `#[cfg(test)]` items.
- `append_to_bib_file` deduplicates by **DOI**, not by key. Same DOI → skip. Same key from different DOI → suffix.
- Unpaywall errors must never cause `fetch_work` to return `Err`. Log warning, return work without OA fields.
- `cargo clippy -- -D warnings` must stay clean before any commit. Run `cargo clippy --fix --allow-dirty` to auto-apply fixes.
