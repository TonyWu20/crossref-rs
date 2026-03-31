# crossref-rs Project Specification (spec.md)

**Project Name**: crossref-rs  
**Version**: v0.2.0
**Status**: Revised
**Last Updated**: 2026-04-01
**Authors**: TWu20 (Tony Wu)
**Goal**: Refactor the original Nushell Crossref tool into a high-performance, cross-platform Rust implementation that supports both Nushell native plugins and a standard CLI for use in any shell.

## 1. Project Goals and Vision

### 1.1 Core Objectives

- Provide a unified command-line tool for literature metadata querying and BibTeX management.
- Deliver a single binary (or multiple binaries) that supports:
  - Nushell native plugin (structured data, pipeline-friendly)
  - Standard CLI for any shell (zsh, fish, bash, PowerShell, etc.)
- Fully preserve and enhance the core functionality of the original Nushell script.
- **Strong emphasis on first-time user experience** to lower the configuration barrier.
- Prioritize **performance**, **reliability**, **usability**, **maintainability**, and **extensibility**.
- Comply with Crossref Polite Usage guidelines, including caching, configuration, and proper error handling.

## 2. Functional Requirements

### 2.1 Core Commands

All commands support the following common options:

- `--format` / `-f`: `table` (default), `json`, `bibtex`, `yaml`, `fzf`
- `--config`: Path to config file (default: `~/.config/crossref.toml`)
- `--no-cache`: Disable cache
- `--verbose` / `-v`: Verbose output
- `--email`: Override polite email (highest priority)

#### 2.1.1 `crossref fetch-meta <DOI>`

- Input: Single DOI or list of DOIs
- Output: Literature metadata (Record / Table)
- Enhancement: Include Unpaywall OA information (`is_oa`, `oa_status`, `pdf_url`)

#### 2.1.2 `crossref fetch-bib <DOI>`

- Input: Single or multiple DOIs
- Output: BibTeX string or save to file
- Enhancements:
  - Intelligent citation key generation (AuthorYear + title abbreviation + conflict suffix a/b/c)
  - `--append <file.bib>`: Automatically deduplicate and append
  - `--key-style`: `author-year` (default), `short-title`, etc.

#### 2.1.3 `crossref search`

- Rich filters:
  - `--title`, `--author`, `--year-from`, `--year-to`
  - `--type` (journal-article, book, etc.)
  - `--open-access`, `--rows`, `--sort`
- Output: Structured table (supports piping: `| where score > 80 | crossref fetch-bib`)

#### 2.1.4 `crossref pdf <DOI>`

- Return the best available PDF link (Unpaywall OA → CityU EZproxy → DOI landing page)

#### 2.1.5 Helper Commands

- `crossref config`: View/edit configuration
- `crossref cache clear`
- `crossref version`

### 2.2 Configuration Loading and First-Run Guidance (Key Feature)

#### 2.2.1 Configuration Priority (Strict Order)

1. **Command-line arguments** (`--email`, `--config`, etc.) — highest priority
2. **Environment variables** (`CROSSREF_EMAIL`, `CROSSREF_PROXY`, `CROSSREF_ROWS`, `CROSSREF_CACHE_TTL_DAYS`, `CROSSREF_DEFAULT_FORMAT`, `CROSSREF_FUZZY_FINDER`, etc.)
3. **Configuration file** (`~/.config/crossref.toml` or path specified by `--config`)
4. **Built-in defaults**

#### 2.2.2 First-Run Smart Guidance (CLI-only)

**Enabled only in the standard CLI binary (`crossref`)**. The Nushell plugin does **not** trigger automatic creation.

**Trigger Condition**:

- No `email` provided via command-line argument, **and**
- `CROSSREF_EMAIL` environment variable is not set, **and**
- Config file does not exist or `email` field is empty/missing

**Behavior**:

- Automatically create a default configuration file at the standard XDG path (`~/.config/crossref.toml`)
- The generated file contains **detailed comments** (in both English and Chinese) explaining each field
- Display a **prominent, well-formatted guidance message** in the terminal including:
  - Full path of the created config file
  - Strong recommendation to edit the `email` field immediately
  - Instructions on how to configure via **environment variables** for different shells
  - Example commands for Bash/Zsh, Fish, Nushell, and PowerShell
  - Message: “Edit the file and re-run your command to continue.”
- After showing guidance, exit gracefully (exit code 0) without executing the original command

**Default Config Template (key sections)**:

```toml
# crossref-rs Default Configuration File
# Auto-generated on {timestamp}

# [REQUIRED] Email for Crossref API (polite usage)
# Replace with your real email to avoid rate limiting
email = "your.name@example.com"

# CityU EZproxy (commonly used by users in Hong Kong)
proxy = "doi-org.ezproxy.cityu.edu.hk"

# Default number of search results
default_rows = 10

# Cache expiration in days
cache_ttl_days = 30

# Optional: custom cache directory
# cache_dir = "/path/to/cache"

# Default output format (table, json, yaml, bibtex, fzf)
# default_format = "table"

# Fuzzy finder program for interactive selection (default: fzf)
# fuzzy_finder_cmd = "fzf"
```

**Terminal Guidance Example** (with color support):

```text
╔══════════════════════════════════════════════════════════════╗
║                 crossref-rs First-Run Setup                  ║
╟──────────────────────────────────────────────────────────────╢
║ A default configuration file has been created for you at:    ║
║   ~/.config/crossref.toml                                    ║
║                                                              ║
║ Please open it now and set your email address:               ║
║   email = "your.real.email@example.com"                      ║
║                                                              ║
║ Alternatively, set via environment variable (quick setup):   ║
║   • Bash / Zsh   : export CROSSREF_EMAIL=you@example.com     ║
║   • Fish         : set -gx CROSSREF_EMAIL you@example.com    ║
║   • Nushell      : $env.CROSSREF_EMAIL = "you@example.com"   ║
║   • PowerShell   : $env:CROSSREF_EMAIL = "you@example.com"   ║
║                                                              ║
║ After editing, re-run your command.                          ║
╚══════════════════════════════════════════════════════════════╝
```

#### 2.2.3 Nushell Plugin Configuration Handling

- **Does not** auto-create the config file
- When critical configuration (email) is missing, output a clear error message guiding the user to set the environment variable or create the config manually

## 3. Technical Architecture

### 3.1 Project Structure

```
crossref-rs/
├── Cargo.toml
├── README.md
├── spec.md
├── src/
│   ├── lib.rs               # Public library interface
│   ├── models.rs            # Data models (Work, BibEntry, etc.)
│   ├── client.rs            # API client abstraction
│   ├── cache.rs             # Caching layer
│   ├── bibtex.rs            # BibTeX generation & parsing
│   ├── config.rs            # Config loading + first-run guidance
│   ├── utils.rs             # Pure utility functions
│   └── error.rs             # Custom error types
├── src/bin/
│   ├── cli.rs               # Standard CLI binary
│   └── nu_plugin_crossref.rs # Nushell plugin binary
├── tests/                   # Unit & integration tests
└── benches/                 # Optional performance benchmarks
```

### 3.2 Key Dependencies

- `directories` / `dirs`: Cross-platform config directory detection
- `confy` or `config`: Config file loading
- `colored`: Beautiful guidance message
- `chrono`: Timestamp in generated config (optional)
- Plus all previously listed crates (reqwest, clap, nu-plugin, etc.)

### 3.3 Config Module Responsibilities (`config.rs`)

- Merge command-line args + environment variables + config file
- Provide `Config::load()` and `Config::load_with_guidance()`
- Implement `create_default_config()` and `print_first_run_guidance()`

## 4. Non-Functional Requirements

### 4.1 Code Quality & Maintainability (Critical)

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

## 5. Development Standards and Workflow

### 5.1 Code Structure Principles

- **Modular design**: Every file and module must follow SRP
- **Clear separation of concerns** (API, config, cache, bibtex, CLI, plugin)
- **No god modules or god functions**
- All public APIs in `lib.rs` must be well-documented with Rust doc comments
- Every complex struct **must** derive `Builder` via `derive_builder`.
- Enforce **builder pattern** and **functional style** in every new struct and algorithm

### 5.2 Test-Driven Development (TDD) Mandate

- All new features or bug fixes must be accompanied by tests written **before** the implementation code
- Use `#[cfg(test)]` modules colocated with the code under test
- Mock external HTTP calls with `wiremock` or `httpmock`
- Run `cargo test` before every commit

### 5.3 Code Style & Tooling

- Enforce `rustfmt` and `clippy --all-targets -- -D warnings`
- Use `cargo clippy` and `cargo fmt` in CI
- Conventional Commits for all git messages
- Semantic Versioning (SemVer)

### 5.4 Git Workflow

- `main` branch is always stable and releasable
- Feature branches prefixed with `feat/`, `fix/`, `refactor/`, `test/`
- Every PR must:
  - Pass all tests (including TDD tests)
  - Update this `spec.md` if requirements change
  - Include relevant documentation updates

## 6. Installation & Usage Examples

(Examples will be expanded in README.md)

## 7. Roadmap

- Phase 1: Core commands + first-run guidance + caching + configuration + **SRP/TDD/Builder/FP foundation**
- Phase 2: Deep Unpaywall integration, smart key generation, etc.
- Phase 3 (Current): Fuzzy finder integration (`--format fzf`), configurable default output format, configurable fuzzy finder program

## 8. Contribution Guidelines

- Fork → Feature branch → PR
- All new code must adhere to the modular SRP + TDD + Builder pattern + Functional style rules above
- Keep this `spec.md` updated with any changes

## 9. License

MIT or Apache-2.0 (to be confirmed)
