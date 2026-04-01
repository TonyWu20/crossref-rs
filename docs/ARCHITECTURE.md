# Architecture notes

## `crossref` crate internals

- Uses `crossref::Crossref` (not `Send`), so it must be built inside `tokio::task::spawn_blocking` every call.
- Override the base URL for testing by setting `client.base_url` after calling `.build()` — the field is `pub`.
- Work path is `/works/{doi}` with **raw slashes** (no percent-encoding).
- `WorkList` response requires `query: { "start-index": 0, "search-terms": "..." }` — both fields are non-optional in the deserialization.
- `WorksFilter::FromPubDate` / `UntilPubDate` take `chrono::NaiveDate`.
- Use `crossref::Type::from_str` for known type strings; fall back to `WorksFilter::TypeName` for unknown strings.
- Open-access filter is approximated with `WorksFilter::HasLicense`.
- **Known crate bug**: `FieldQuery::param_key()` returns `"title"` instead of `"query.title"`, causing the Crossref REST API to return `validation-failure`. **Do not use `FieldQuery`**. Combine all text inputs (`query`, `title`, `author`) into the free-form `query=` parameter via `WorksQuery::new(term)`. Use `WorksQuery::empty()` when no text is provided (passing an empty string to `WorksQuery::new` also triggers validation-failure).

## Unpaywall enrichment

- `fetch_work` auto-calls `fetch_unpaywall` and merges `is_oa`, `oa_status`, `pdf_url` into `WorkMeta`.
- Unpaywall failures are **non-fatal**: a warning is printed to stderr and the work is returned without OA fields.
- Unpaywall URL pattern: `{base}/{doi}?email={email}`. Default base: `https://api.unpaywall.org/v2`.

## `CrossrefClient` construction

- `CrossrefClient::new(Arc<Config>)` — production use.
- `CrossrefClient::new_with_base_urls(cfg, crossref_url, unpaywall_url)` — always public, used by integration tests to inject mock server URLs.

## Configuration priority (highest → lowest)

1. CLI `--email` flag
2. `CROSSREF_EMAIL` env var
3. `~/.config/crossref.toml` (or `--config FILE`)
4. Built-in defaults

**Output format resolution** (highest → lowest):

1. CLI `--format` flag
2. `CROSSREF_DEFAULT_FORMAT` env var
3. `default_format` in config file
4. Built-in default: `"table"`

**Fuzzy finder command** (highest → lowest):

1. `CROSSREF_FUZZY_FINDER` env var
2. `fuzzy_finder_cmd` in config file
3. Built-in default: `"fzf"`

Config TOML keys: `email`, `proxy`, `default_rows`, `cache_ttl_days`, `cache_dir`, `default_format`, `fuzzy_finder_cmd`.

## PDF download

- `download_pdf` checks that the response body starts with the PDF magic bytes `%PDF-` before writing to disk.
- Publishers frequently return HTML landing pages (status 200) at OA PDF URLs; the magic-byte check prevents saving garbage HTML.
- Helper: `fn is_pdf(bytes: &[u8]) -> bool { bytes.starts_with(b"%PDF-") }` in `src/client.rs`.
- Download attempt order: (1) direct Unpaywall `url_for_pdf`, (2) EZproxy fallback (`config.proxy`), (3) return `Err(CrossrefError::PdfDownload(...))`.

## Cache

- `ttl_days = 0` disables the cache entirely.
- Cache keys: bare DOI for single works; `"search:" + serde_json::to_string(&query)` for search results.
- Key-to-filename sanitisation: non-alphanumeric chars (except `-`, `_`, `.`) → `_`.
- Cache failures are non-fatal; errors are silently swallowed in the CLI handlers.

## BibTeX

- `work_to_bib_record` → AuthorYear style.
- `work_to_bib_record_with_style(work, &KeyStyle)` → caller-specified style.
- `append_to_bib_file` performs **DOI-based dedup** (idempotent across calls) and **key conflict resolution** within each batch using letter suffixes (`a`–`z`, then `aa`–`zz`).
- `BibRecord.fields` is `BTreeMap<String, String>` — output is deterministically ordered.
- `issue` from Crossref maps to BibTeX field `number`.

## Citation key styles

- `KeyStyle::AuthorYear` → `{FamilyName(s up to 2)}{Year}`, e.g. `SmithJones2024`.
- `KeyStyle::ShortTitle` → up to 4 significant title words (stop-words stripped) + year, e.g. `MachineLearning2024`.
- Stop-word list lives in `utils::generate_short_title_key`.
- Conflict resolution: `Smith2024` → `Smith2024a` → … → `Smith2024z` → `Smith2024aa` → …

## Fuzzy finder output (`--format fzf`)

- Outputs one line per result, tab-separated: `DOI\tTitle\tAuthors\tYear\tJournal\tOA`.
- No decoration characters, no header — designed for piping to `fzf`, `skim`, or similar programs.
- OA field shows `"OA"` for open-access items, empty string otherwise.
- The `--format` flag defaults to `"table"` but can be overridden globally via `default_format` in config or `CROSSREF_DEFAULT_FORMAT` env var.
- The fuzzy finder program is configurable via `fuzzy_finder_cmd` in config or `CROSSREF_FUZZY_FINDER` env var (default: `fzf`).
