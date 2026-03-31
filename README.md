# crossref-rs

Query Crossref literature metadata, generate BibTeX entries, and download open-access PDFs — from the command line or inside a Nushell pipeline.

[![Crates.io](https://img.shields.io/crates/v/crossref-rs)](https://crates.io/crates/crossref-rs)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE)

## Features

- Fetch Crossref metadata by DOI with automatic [Unpaywall](https://unpaywall.org) OA enrichment
- Free-text and filtered search (year range, work type, open-access)
- PDF download via direct OA URL or EZproxy fallback
- BibTeX generation with `--key-style author-year|short-title`
- Automatic citation key conflict resolution (`Smith2024`, `Smith2024a`, …)
- Append to `.bib` file with DOI-based deduplication
- Disk response cache with configurable TTL (`--no-cache` to bypass)
- Multiple output formats: `table` (default), `json`, `yaml`, `bibtex`
- Nushell plugin with pipeline-friendly structured output

## Installation

```bash
cargo install crossref-rs
```

From source:

```bash
git clone https://github.com/TonyWu20/crossref-rs
cd crossref-rs
cargo install --path .
```

For the Nushell plugin, register it after installation:

```nu
register (which nu_plugin_crossref | get path.0)
```

## Configuration

On first invocation without a configured email, `crossref` creates a default config file and prints shell-specific setup instructions. Edit the file to add your email, then re-run.

**Config file:** `~/.config/crossref.toml` (override with `--config FILE`)

```toml
email = "your.name@institution.edu"      # required for Crossref polite pool
proxy = "doi-org.ezproxy.example.edu"    # optional EZproxy host for PDF fallback
default_rows = 10
cache_ttl_days = 30
# cache_dir = "/custom/cache/path"
```

**Priority** (highest to lowest):

1. `--email` CLI flag
2. `CROSSREF_EMAIL` environment variable
3. Config file
4. Built-in defaults

**Other environment variables:** `CROSSREF_PROXY`, `CROSSREF_ROWS`, `CROSSREF_CACHE_TTL_DAYS`

## CLI Usage

### Global flags

```
--config FILE        Path to config file
--email EMAIL        Override polite email  (env: CROSSREF_EMAIL)
--no-cache           Disable cache for this invocation
-f, --format FORMAT  Output format: table (default), json, yaml, bibtex
-v, --verbose        Print cache hits/misses and API details to stderr
```

### fetch-meta

Fetch metadata for one or more DOIs.

```bash
crossref fetch-meta 10.1038/nature12373
crossref fetch-meta 10.1038/nature12373 --format json
crossref fetch-meta 10.1038/nature12373 10.1126/science.aaf7680
```

### fetch-bib

Fetch BibTeX entries for one or more DOIs.

```bash
crossref fetch-bib 10.1038/nature12373
crossref fetch-bib 10.1038/nature12373 --key-style short-title

# Append to a .bib file (deduplicates by DOI, resolves key conflicts)
crossref fetch-bib 10.1038/nature12373 10.1126/science.1245679 --append refs.bib
```

### search

Search Crossref for literature.

```bash
crossref search "transformer neural networks" --year-from 2020 --type journal-article --rows 5
crossref search --author "LeCun" --open-access --format json
crossref search "CRISPR" --year-from 2018 --year-to 2023 --sort published
```

Options:

```
QUERY                    Free-text query
--title TITLE            Filter by title
--author AUTHOR          Filter by author
--year-from YEAR         Earliest publication year
--year-to YEAR           Latest publication year
--type TYPE              Work type (e.g. journal-article, book-chapter)
--open-access            Only return open-access items
-n, --rows N             Number of results (default: 10)
--sort ORDER             Score (default), updated, deposited, indexed, published
```

### pdf

Download the best available OA PDF, or fall back to an EZproxy URL or a doi.org link.

```bash
crossref pdf 10.1038/nature12373
crossref pdf 10.1038/nature12373 --output ~/papers/
```

### config

```bash
crossref config path    # print path to active config file
crossref config show    # print current effective configuration (JSON)
```

### cache

```bash
crossref cache prune    # remove expired entries
crossref cache clear    # wipe all cached entries
```

## Output Formats

| Flag value        | Description                                |
| ----------------- | ------------------------------------------ |
| `table` (default) | Human-readable terminal table              |
| `json`            | Pretty-printed JSON                        |
| `yaml`            | YAML                                       |
| `bibtex`          | BibTeX entries (`fetch-meta`, `fetch-bib`) |

## Nushell Plugin

After registration, all commands return structured data and compose naturally with Nushell pipelines.

```nu
# Fetch metadata — returns a record
crossref fetch-meta "10.1038/nature12373"

# Search — returns a list of records
crossref search "deep learning" --rows 20
    | where is_oa == true
    | select doi title year

# BibTeX string
crossref fetch-bib "10.1038/nature12373"

# Download PDF
crossref pdf "10.1038/nature12373" --output ~/papers/
```

All plugin commands accept `--email` to override the configured address for that call.

## Library Usage

`crossref_lib` is the underlying Rust library and can be used directly:

```rust
use std::sync::Arc;
use crossref_lib::{client::CrossrefClient, config::Config, models::SearchQueryBuilder};

#[tokio::main]
async fn main() -> crossref_lib::Result<()> {
    let cfg = Arc::new(Config::load(Some("me@example.com"), None)?);
    let client = CrossrefClient::new(cfg)?;

    // Fetch one work (auto-enriched with Unpaywall OA data)
    let work = client.fetch_work("10.1038/nature12373").await?;
    println!("{:?}", work);

    // Search
    let query = SearchQueryBuilder::default()
        .query(Some("machine learning".into()))
        .rows(5u32)
        .build()?;
    let results = client.search(&query).await?;
    println!("Found {} results", results.total_results);
    Ok(())
}
```

## BibTeX Details

### Citation key styles

| Style                 | Flag                      | Example                       |
| --------------------- | ------------------------- | ----------------------------- |
| Author-year (default) | `--key-style author-year` | `Smith2024`, `SmithJones2024` |
| Short-title           | `--key-style short-title` | `MachineLearning2024`         |

Short-title strips common stop-words (a, the, of, in, …) and takes up to four significant title words. Key conflicts are resolved automatically by appending letter suffixes: `Smith2024`, `Smith2024a`, `Smith2024b`, …

### Entry type mapping

| Crossref type       | BibTeX type   |
| ------------------- | ------------- |
| journal-article     | article       |
| book, monograph     | book          |
| book-chapter        | inbook        |
| proceedings-article | inproceedings |
| dissertation        | phdthesis     |
| report              | techreport    |
| (other)             | misc          |

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.
