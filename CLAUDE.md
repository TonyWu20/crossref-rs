# CLAUDE.md — crossref-rs

## Project overview

`crossref-rs` is a Crossref literature metadata and BibTeX management tool built in Rust. It exposes three compilation targets from one workspace:

| Target | Binary | Entry point |
|---|---|---|
| Shared library | `crossref_lib` | `src/lib.rs` |
| Standard CLI | `crossref` | `src/bin/cli.rs` |
| Nushell plugin | `nu_plugin_crossref` | `src/bin/nu_plugin_crossref.rs` |

Both binaries depend exclusively on `crossref_lib`. No logic lives in the binary entry points beyond argument parsing and output formatting.

## Common commands

```bash
cargo build                        # build all targets (nushell 0.111 plugin, default)
cargo build --no-default-features --features nu-v110  # build with nushell 0.110 plugin
cargo test                         # run all unit + integration tests (52 tests)
cargo clippy -- -D warnings        # lint — must stay clean
cargo clippy --fix --allow-dirty   # auto-fix lint issues
cargo check                        # fast type-check without linking
```

## Project structure

```
src/
  lib.rs                   re-exports: CrossrefClient, Config, CrossrefError,
                           Result, WorkMeta, BibRecord, SearchQuery,
                           SearchResult, KeyStyle
  client.rs                CrossrefClient (Crossref + Unpaywall HTTP)
  config.rs                Config struct, load priority, first-run guidance
  cache.rs                 DiskCache (JSON files, TTL-based expiry)
  models.rs                WorkMeta, BibRecord, SearchQuery, SearchResult
                           (all derive derive_builder + serde)
  utils.rs                 citation key generation, DOI normalisation,
                           key conflict resolution
  bibtex.rs                work_to_bib_record*, records_to_bibtex,
                           append_to_bib_file (dedup + conflict resolution)
  error.rs                 CrossrefError enum, Result type alias
  bin/
    cli.rs                 clap CLI, all subcommand handlers
    nu_plugin_crossref.rs  nu-plugin 0.111 plugin

tests/
  client_tests.rs          async wiremock tests for all CrossrefClient methods
  bibtex_tests.rs          BibTeX conversion, key styles, append dedup
  cache_tests.rs           DiskCache set/get/TTL/clear
  config_tests.rs          Config priority, env overrides, first-run
```

## Architecture notes

### `crossref` crate internals
- Uses `crossref::Crossref` (not `Send`), so it must be built inside `tokio::task::spawn_blocking` every call.
- Override the base URL for testing by setting `client.base_url` after calling `.build()` — the field is `pub`.
- Work path is `/works/{doi}` with **raw slashes** (no percent-encoding).
- `WorkList` response requires `query: { "start-index": 0, "search-terms": "..." }` — both fields are non-optional in the deserialization.
- `WorksFilter::FromPubDate` / `UntilPubDate` take `chrono::NaiveDate`.
- Use `crossref::Type::from_str` for known type strings; fall back to `WorksFilter::TypeName` for unknown strings.
- Open-access filter is approximated with `WorksFilter::HasLicense`.
- **Known crate bug**: `FieldQuery::param_key()` returns `"title"` instead of `"query.title"`, causing the Crossref REST API to return `validation-failure`. **Do not use `FieldQuery`**. Combine all text inputs (`query`, `title`, `author`) into the free-form `query=` parameter via `WorksQuery::new(term)`. Use `WorksQuery::empty()` when no text is provided (passing an empty string to `WorksQuery::new` also triggers validation-failure).

### Unpaywall enrichment
- `fetch_work` auto-calls `fetch_unpaywall` and merges `is_oa`, `oa_status`, `pdf_url` into `WorkMeta`.
- Unpaywall failures are **non-fatal**: a warning is printed to stderr and the work is returned without OA fields.
- Unpaywall URL pattern: `{base}/{doi}?email={email}`. Default base: `https://api.unpaywall.org/v2`.

### `CrossrefClient` construction
- `CrossrefClient::new(Arc<Config>)` — production use.
- `CrossrefClient::new_with_base_urls(cfg, crossref_url, unpaywall_url)` — always public, used by integration tests to inject mock server URLs.

### Configuration priority (highest → lowest)
1. CLI `--email` flag
2. `CROSSREF_EMAIL` env var
3. `~/.config/crossref.toml` (or `--config FILE`)
4. Built-in defaults

Config TOML keys: `email`, `proxy`, `default_rows`, `cache_ttl_days`, `cache_dir`.

### PDF download
- `download_pdf` checks that the response body starts with the PDF magic bytes `%PDF-` before writing to disk.
- Publishers frequently return HTML landing pages (status 200) at OA PDF URLs; the magic-byte check prevents saving garbage HTML.
- Helper: `fn is_pdf(bytes: &[u8]) -> bool { bytes.starts_with(b"%PDF-") }` in `src/client.rs`.
- Download attempt order: (1) direct Unpaywall `url_for_pdf`, (2) EZproxy fallback (`config.proxy`), (3) return `Err(CrossrefError::PdfDownload(...))`.

### Cache
- `ttl_days = 0` disables the cache entirely.
- Cache keys: bare DOI for single works; `"search:" + serde_json::to_string(&query)` for search results.
- Key-to-filename sanitisation: non-alphanumeric chars (except `-`, `_`, `.`) → `_`.
- Cache failures are non-fatal; errors are silently swallowed in the CLI handlers.

### BibTeX
- `work_to_bib_record` → AuthorYear style.
- `work_to_bib_record_with_style(work, &KeyStyle)` → caller-specified style.
- `append_to_bib_file` performs **DOI-based dedup** (idempotent across calls) and **key conflict resolution** within each batch using letter suffixes (`a`–`z`, then `aa`–`zz`).
- `BibRecord.fields` is `BTreeMap<String, String>` — output is deterministically ordered.
- `issue` from Crossref maps to BibTeX field `number`.

### Citation key styles
- `KeyStyle::AuthorYear` → `{FamilyName(s up to 2)}{Year}`, e.g. `SmithJones2024`.
- `KeyStyle::ShortTitle` → up to 4 significant title words (stop-words stripped) + year, e.g. `MachineLearning2024`.
- Stop-word list lives in `utils::generate_short_title_key`.
- Conflict resolution: `Smith2024` → `Smith2024a` → … → `Smith2024z` → `Smith2024aa` → …

## Testing patterns

### Integration tests (tests/)
- HTTP mocking via `wiremock` (not `mockito`). Use `MockServer::start().await`.
- Crossref single-work mock: `path(format!("/works/{doi}"))` — raw DOI, **no URL encoding**.
- Crossref search mock: `path_regex(r"^/works")` (search uses query params, not path segments).
- A minimal but valid `Work` JSON must include: `publisher`, `title`, `DOI`, `URL`, `type`, `prefix`, `member`, `source`, `references-count`, `is-referenced-by-count`, `indexed` (with `date-parts`, `date-time`, `timestamp`), `deposited` (same), `issued`, `author` (array with `family`, `given`, `sequence`, `affiliation`), `container-title`, `content-domain`.
- Works-list JSON requires `query: { "start-index": 0, "search-terms": "..." }` in the `message` object.
- Tests that touch env vars must serialize on `static ENV_MUTEX: Mutex<()>` to avoid races.

### Unit tests (`#[cfg(test)]` in source files)
- Prefer `tempfile::NamedTempFile` / `tempfile::tempdir()` for any filesystem operations.
- `WorkMeta::default()` is available — use `..WorkMeta::default()` struct update syntax in fixtures.

## Key invariants to preserve

- `CrossrefClient::new_with_base_urls` must remain `pub` (not `#[cfg(test)]`) — integration tests in `tests/` are separate crates and cannot see `#[cfg(test)]` items.
- `append_to_bib_file` deduplicates by **DOI**, not by key. Same DOI → skip. Same key from different DOI → suffix.
- Unpaywall errors must never cause `fetch_work` to return `Err`. Log warning, return work without OA fields.
- `cargo clippy -- -D warnings` must stay clean before any commit. Run `cargo clippy --fix --allow-dirty` to auto-apply fixes.

## Short flags reference

### Global (both CLI and subcommands)
| Flag | Short |
|---|---|
| `--config` | `-c` |
| `--email` | `-e` |
| `--format` | `-f` |
| `--verbose` | `-v` |

### `fetch-bib`
| Flag | Short |
|---|---|
| `--append` | `-a` |
| `--key-style` | `-k` |

### `search`
| Flag | Short |
|---|---|
| `--title` | `-t` |
| `--author` | `-a` |
| `--year-from` | `-F` |
| `--year-to` | `-T` |
| `--type` | `-y` |
| `--open-access` | `-A` |
| `--rows` | `-n` |
| `--sort` | `-s` |

### `pdf`
| Flag | Short |
|---|---|
| `--output` | `-o` |

Nu plugin `SearchCommand` carries the same short flags (`-F`, `-T`, `-y`, `-A`; `-e`, `-t`, `-a`, `-n`, `-s` already existed).

## Nushell plugin version selection

The `nu_plugin_crossref` binary can be compiled against two nu-plugin versions:

| Feature flag | Nushell target | Build command |
|---|---|---|
| `nu-v111` (default) | nushell 0.111 | `cargo build` |
| `nu-v110` | nushell 0.110 | `cargo build --no-default-features --features nu-v110` |

**How it works:**
- Both `nu-plugin 0.111` and `nu-plugin 0.110` are optional deps with distinct Cargo names (`nu-plugin-v110`, `nu-protocol-v110` for 0.110).
- `src/bin/nu_plugin_crossref.rs` has `#[cfg(feature = "nu-v110")] extern crate nu_plugin_v110 as nu_plugin;` at the top; the rest of the file is version-agnostic.
- `nu-plugin-core 0.110.0` (a transitive dep) had a broken `interprocess` import (`local_socket::traits::ListenerNonblockingMode` was moved to `local_socket::ListenerNonblockingMode` in interprocess 2.3.x). A local patch at `patches/nu-plugin-core-v110/` applies the same one-line fix that 0.111.0 contains. Cargo uses the patch only for the 0.110.0 dep resolution.

**Key invariant:** Do not remove `[patch.crates-io]` from `Cargo.toml` — it's required for `nu-v110` builds. The patch is in `patches/nu-plugin-core-v110/` and must not be deleted.

## Current state (v0.1.0 — post-Phase 2 fixes)

All 52 tests pass. `cargo clippy -- -D warnings` clean.

Implemented and shipped:
- Search filters forwarded to Crossref API (date range, type, OA)
- Unpaywall auto-enrichment in `fetch_work`
- EZproxy fallback in `download_pdf`
- `--key-style short-title` fully wired
- Citation key conflict resolution in `append_to_bib_file`
- Search result caching in `cmd_search`
- YAML output (`serde_yaml`)
- `comfy-table` table rendering
- `--verbose` flag threaded through command handlers
- Nu plugin search params parity (`--year-from`, `--year-to`, `--type`, `--open-access`, `--sort`)
- `build_works_query` rewritten to avoid `FieldQuery` crate bug (validation-failure fix)
- `download_pdf` PDF magic-byte check (prevents saving HTML landing pages)
- Short flags added to CLI and Nu plugin (`-c`, `-e`, `-F`, `-T`, `-y`, `-A`, `-k`, `-a`, `-s`, `-t`)
