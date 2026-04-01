# CLAUDE.md — crossref-rs

## Project overview

`crossref-rs` is a Crossref literature metadata and BibTeX management tool built in Rust. It exposes three compilation targets from one workspace:

| Target         | Binary               | Entry point                     |
| -------------- | -------------------- | ------------------------------- |
| Shared library | `crossref_lib`       | `src/lib.rs`                    |
| Standard CLI   | `crossref`           | `src/bin/cli.rs`                |
| Nushell plugin | `nu_plugin_crossref` | `src/bin/nu_plugin_crossref.rs` |

Both binaries depend exclusively on `crossref_lib`. No logic lives in the binary entry points beyond argument parsing and output formatting.

## Code style

- **Modular architecture** strictly following the **Single Responsibility Principle (SRP)**: each module, struct, and function has exactly one reason to change.
- **Test-Driven Development (TDD)** is mandatory for all core logic:
  - Write failing tests first
  - Implement the minimal code to make tests pass
  - Refactor while keeping tests green
- Unit test coverage target: ≥ 85% for `lib/` modules
- Integration tests for CLI and plugin entry points
- **Builder pattern** **must** be used for all complex struct creation.
  - Use the **`derive_builder`** crate (`#[derive(Builder)]`) for all non-trivial structs (`Config`, `SearchQuery`, `BibEntry`, `Work`, etc.).
  - This provides clean, ergonomic, immutable builders with method chaining.
- **Functional programming style** must be used as much as possible:
  - Prefer iterators (`iter()`, `map`, `filter`, `filter_map`, `flat_map`, `fold`, `collect`, etc.) over imperative `for` loops.
  - Minimize mutable state; favor immutable transformations and method chaining.
  - Use higher-order functions and combinators wherever they improve clarity and reduce side effects.

## Important docs

Read when necessary.

- ./docs/PROJECT_STRUCTURE.md
- ./docs/ARCHITECTURE.md
- ./docs/TESTING_PATTERNS.md

## Common commands

```bash
cargo build                        # build all targets (nushell 0.111 plugin, default)
cargo build --no-default-features --features nu-v110  # build with nushell 0.110 plugin
cargo test                         # run all unit + integration tests (52 tests)
cargo clippy -- -D warnings        # lint — must stay clean
cargo clippy --fix --allow-dirty   # auto-fix lint issues
cargo check                        # fast type-check without linking
```

## Current state (v0.2.0)

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
- **`--format fzf`**: fuzzy-finder-friendly plain text output (tab-separated, one line per result)
- **Configurable default output format** via `default_format` config key / `CROSSREF_DEFAULT_FORMAT` env var
- **Configurable fuzzy finder program** via `fuzzy_finder_cmd` config key / `CROSSREF_FUZZY_FINDER` env var (default: `fzf`)
