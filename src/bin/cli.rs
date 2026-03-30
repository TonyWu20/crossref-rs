// src/bin/cli.rs
// CLI binary for crossref-rs (standard shell usage: zsh, fish, bash, PowerShell, etc.)
//
// This file follows the spec.md (v0.3.0):
// - Single Responsibility Principle: ONLY argument parsing, first-run guidance dispatch,
//   command routing, and output formatting.
// - All business logic lives in the shared library (`crossref_lib`).
// - TDD-ready: includes `debug_assert()` verification + example parse tests.
// - Clap v4+ best practices (derive macros + CommandFactory for testing).

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crossref_lib::{
    config::{self, Config}, // handles loading + first-run guidance (CLI-only)
    // Core command handlers (all async, return structured data)
    fetch_bib, fetch_meta, pdf_link, run_search,
    // Output helper (SRP: formatting + printing)
    output::{print_output, OutputFormat},
};

/// crossref-rs – Literature metadata & BibTeX tool
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(flatten)]
    global: GlobalOpts,

    #[command(subcommand)]
    command: Command,
}

/// Global options available to every subcommand
#[derive(Parser)]
struct GlobalOpts {
    /// Output format
    #[arg(long, short, default_value_t = OutputFormat::Table)]
    format: OutputFormat,

    /// Path to config file (default: ~/.config/crossref.toml)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Override polite email (highest priority)
    #[arg(long)]
    email: Option<String>,

    /// Disable cache for this run
    #[arg(long)]
    no_cache: bool,

    /// Verbose output (-v, -vv, …)
    #[arg(long, short, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Command {
    /// Fetch metadata for one or more DOIs
    FetchMeta {
        /// DOI(s) to fetch (space-separated or repeated)
        #[arg(required = true)]
        doi: Vec<String>,
    },

    /// Fetch BibTeX for one or more DOIs
    FetchBib {
        /// DOI(s) to fetch
        #[arg(required = true)]
        doi: Vec<String>,

        /// Append to existing .bib file (deduplicates automatically)
        #[arg(long)]
        append: Option<PathBuf>,

        /// Citation key style
        #[arg(long, default_value = "author-year")]
        key_style: String,
    },

    /// Search Crossref with rich filters
    Search {
        /// Title contains
        #[arg(long)]
        title: Option<String>,

        /// Author contains
        #[arg(long)]
        author: Option<String>,

        /// Year range
        #[arg(long)]
        year_from: Option<u32>,
        #[arg(long)]
        year_to: Option<u32>,

        /// Publication type filter
        #[arg(long)]
        r#type: Option<String>,

        /// Open-access only
        #[arg(long)]
        open_access: bool,

        /// Max results
        #[arg(long, default_value_t = 10)]
        rows: u32,
    },

    /// Get best PDF link (Unpaywall → EZproxy → DOI page)
    Pdf {
        /// DOI to resolve
        doi: String,
    },

    /// Show or edit configuration
    Config {
        /// Show current effective config
        #[arg(long)]
        show: bool,
    },

    /// Cache management
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// Print version information
    Version,
}

#[derive(Subcommand)]
enum CacheAction {
    /// Clear the entire cache
    Clear,
}

#[tokio::main]
async fn main() {
    // Let Clap handle help/version/errors automatically
    if let Err(err) = try_main().await {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

async fn try_main() -> Result<()> {
    let cli = Cli::parse();

    // === FIRST-RUN GUIDANCE (CLI-only) ===
    // This call follows spec.md §2.2.2 exactly:
    // - Checks env + config + CLI email
    // - If missing → creates ~/.config/crossref.toml + colorful guidance
    // - Prints message and exits(0) gracefully
    let config: Config = config::load_with_guidance(&cli.global)?;

    // === DISPATCH TO LIBRARY (zero business logic here) ===
    match cli.command {
        Command::FetchMeta { doi } => {
            let result = fetch_meta(&doi, &config).await?;
            print_output(result, cli.global.format);
        }

        Command::FetchBib { doi, append, key_style } => {
            let result = fetch_bib(&doi, &config, append, &key_style).await?;
            print_output(result, cli.global.format);
        }

        Command::Search {
            title,
            author,
            year_from,
            year_to,
            r#type,
            open_access,
            rows,
        } => {
            let result = run_search(
                title,
                author,
                year_from,
                year_to,
                r#type,
                open_access,
                rows,
                &config,
            )
            .await?;
            print_output(result, cli.global.format);
        }

        Command::Pdf { doi } => {
            let result = pdf_link(&doi, &config).await?;
            print_output(result, cli.global.format);
        }

        Command::Config { show } => {
            if show {
                println!("{:#?}", config);
            } else {
                println!("Use `crossref config --show` or edit ~/.config/crossref.toml");
            }
        }

        Command::Cache { action: CacheAction::Clear } => {
            crossref_lib::cache::clear()?;
            println!("Cache cleared successfully.");
        }

        Command::Version => {
            println!("crossref-rs {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

// ==================== TDD / TEST SECTION ====================
// These tests are written FIRST (TDD) and use Clap's official testing support.
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// Clap's official debug_assert! verification (catches definition errors at test time)
    /// See: https://docs.rs/clap/latest/clap/struct.Command.html#method.debug_assert
    #[test]
    fn verify_cli_definition() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_fetch_meta() {
        let cli = Cli::parse_from(["crossref", "fetch-meta", "10.1145/1234567"]);
        assert!(matches!(cli.command, Command::FetchMeta { .. }));
    }

    #[test]
    fn parses_global_flags() {
        let cli = Cli::parse_from([
            "crossref",
            "--format",
            "json",
            "--email",
            "test@example.com",
            "--no-cache",
            "-vv",
            "fetch-bib",
            "10.1145/1234567",
        ]);
        assert_eq!(cli.global.format, OutputFormat::Json);
        assert_eq!(cli.global.email, Some("test@example.com".into()));
        assert!(cli.global.no_cache);
        assert_eq!(cli.global.verbose, 2);
    }

    #[test]
    fn parses_search_with_filters() {
        let cli = Cli::parse_from([
            "crossref",
            "search",
            "--title",
            "nushell",
            "--year-from",
            "2020",
            "--open-access",
        ]);
        if let Command::Search { title, year_from, open_access, .. } = cli.command {
            assert_eq!(title, Some("nushell".into()));
            assert_eq!(year_from, Some(2020));
            assert!(open_access);
        } else {
            panic!("wrong subcommand");
        }
    }
}
