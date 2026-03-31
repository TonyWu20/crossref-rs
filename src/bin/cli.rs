use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use comfy_table::{Cell, Table};

use crossref_lib::{
    bibtex::{append_to_bib_file, records_to_bibtex, work_to_bib_record, work_to_bib_record_with_style},
    cache::DiskCache,
    client::CrossrefClient,
    config::Config,
    models::SearchQueryBuilder,
    utils::KeyStyle,
};

// ─── CLI definition ───────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "crossref",
    version,
    about = "Query Crossref literature metadata and manage BibTeX entries",
    long_about = None,
)]
struct Cli {
    /// Path to configuration file (default: ~/.config/crossref.toml)
    #[arg(long, short = 'c', global = true, value_name = "FILE")]
    config: Option<String>,

    /// Override polite email for this invocation
    #[arg(long, short = 'e', global = true, env = "CROSSREF_EMAIL", value_name = "EMAIL")]
    email: Option<String>,

    /// Disable response caching
    #[arg(long, global = true)]
    no_cache: bool,

    /// Output format
    #[arg(long, short = 'f', global = true,
          value_parser = ["table", "json", "bibtex", "yaml", "fzf"])]
    format: Option<String>,

    /// Enable verbose output
    #[arg(long, short = 'v', global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch metadata for one or more DOIs
    FetchMeta {
        /// DOI(s) to look up
        #[arg(required = true, value_name = "DOI")]
        dois: Vec<String>,
    },

    /// Fetch BibTeX entry for one or more DOIs
    FetchBib {
        /// DOI(s) to look up
        #[arg(required = true, value_name = "DOI")]
        dois: Vec<String>,

        /// Append to a .bib file (deduplicates automatically)
        #[arg(long, short = 'a', value_name = "FILE")]
        append: Option<PathBuf>,

        /// Citation key style
        #[arg(long, short = 'k', default_value = "author-year",
              value_parser = ["author-year", "short-title"])]
        key_style: String,
    },

    /// Search Crossref for literature
    Search {
        /// Free-text query
        #[arg(value_name = "QUERY")]
        query: Option<String>,

        /// Filter by title
        #[arg(long, short = 't')]
        title: Option<String>,

        /// Filter by author
        #[arg(long, short = 'a')]
        author: Option<String>,

        /// Earliest publication year
        #[arg(long, short = 'F', value_name = "YEAR")]
        year_from: Option<i32>,

        /// Latest publication year
        #[arg(long, short = 'T', value_name = "YEAR")]
        year_to: Option<i32>,

        /// Work type filter (e.g. journal-article)
        #[arg(long, short = 'y', value_name = "TYPE")]
        r#type: Option<String>,

        /// Only return open-access items
        #[arg(long, short = 'A')]
        open_access: bool,

        /// Number of results to return
        #[arg(long, short = 'n', default_value_t = 10)]
        rows: u32,

        /// Sort order (score, updated, deposited, indexed, published)
        #[arg(long, short = 's', default_value = "score")]
        sort: String,
    },

    /// Get the best available PDF link or download the PDF for a DOI
    Pdf {
        /// DOI to look up
        #[arg(value_name = "DOI")]
        doi: String,

        /// Directory to save the downloaded PDF (default: current directory)
        #[arg(long, short = 'o', value_name = "DIR")]
        output: Option<PathBuf>,
    },

    /// View or edit configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage the local response cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// Print version information
    Version,
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Print the path to the active config file
    Path,
    /// Print current effective configuration
    Show,
}

#[derive(Subcommand)]
enum CacheAction {
    /// Remove expired entries from the cache
    Prune,
    /// Remove all cached entries
    Clear,
}

// ─── Entry point ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

async fn run() -> crossref_lib::Result<()> {
    let cli = Cli::parse();

    // First-run guidance (CLI only): exits early if guidance was printed
    let cfg = match Config::load_with_guidance(
        cli.email.as_deref(),
        cli.config.as_deref(),
    )? {
        Some(cfg) => Arc::new(cfg),
        None => std::process::exit(0),
    };

    // Resolve output format: CLI flag > config default_format > "table"
    let format = cli.format
        .or_else(|| cfg.default_format.clone())
        .unwrap_or_else(|| "table".to_string());

    match cli.command {
        Commands::FetchMeta { dois } => {
            cmd_fetch_meta(&format, cli.no_cache, cli.verbose, cfg, &dois).await?;
        }
        Commands::FetchBib { dois, append, key_style } => {
            let style = parse_key_style(&key_style);
            cmd_fetch_bib(&format, cli.no_cache, cli.verbose, cfg, &dois, append, style).await?;
        }
        Commands::Search {
            query,
            title,
            author,
            year_from,
            year_to,
            r#type,
            open_access,
            rows,
            sort,
        } => {
            let search_q = SearchQueryBuilder::default()
                .query(query)
                .title(title)
                .author(author)
                .year_from(year_from)
                .year_to(year_to)
                .work_type(r#type)
                .open_access(open_access)
                .rows(rows)
                .sort(sort)
                .build()
                .map_err(|e| crossref_lib::CrossrefError::Builder(e.to_string()))?;
            cmd_search(&format, cli.no_cache, cli.verbose, cfg, &search_q).await?;
        }
        Commands::Pdf { doi, output } => {
            let dest = output.unwrap_or_else(|| PathBuf::from("."));
            cmd_pdf(cfg, &doi, &dest).await?;
        }
        Commands::Config { action } => {
            cmd_config(action, cli.config.as_deref())?;
        }
        Commands::Cache { action } => {
            cmd_cache(action, &cfg)?;
        }
        Commands::Version => {
            println!("crossref-rs {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

// ─── Command implementations ──────────────────────────────────────────────────

async fn cmd_fetch_meta(
    format: &str,
    no_cache: bool,
    verbose: bool,
    cfg: Arc<Config>,
    dois: &[String],
) -> crossref_lib::Result<()> {
    let client = CrossrefClient::new(cfg.clone())?;
    let cache = if no_cache { None } else { DiskCache::from_config(&cfg).ok() };

    if verbose {
        eprintln!("[verbose] cache: {}", if cache.is_some() { "enabled" } else { "disabled" });
    }

    for doi in dois {
        let work = if let Some(ref c) = cache {
            match c.get::<crossref_lib::WorkMeta>(doi)? {
                Some(cached) => {
                    if verbose {
                        eprintln!("[verbose] cache HIT for {doi}");
                    }
                    cached
                }
                None => {
                    if verbose {
                        eprintln!("[verbose] cache MISS for {doi}");
                    }
                    let w = client.fetch_work(doi).await?;
                    let _ = c.set(doi, &w);
                    w
                }
            }
        } else {
            client.fetch_work(doi).await?
        };

        print_work(&work, format);
    }
    Ok(())
}

async fn cmd_fetch_bib(
    format: &str,
    no_cache: bool,
    verbose: bool,
    cfg: Arc<Config>,
    dois: &[String],
    append: Option<PathBuf>,
    key_style: KeyStyle,
) -> crossref_lib::Result<()> {
    let client = CrossrefClient::new(cfg.clone())?;
    let cache = if no_cache { None } else { DiskCache::from_config(&cfg).ok() };

    if verbose {
        eprintln!("[verbose] cache: {}", if cache.is_some() { "enabled" } else { "disabled" });
    }

    let mut records = Vec::new();
    for doi in dois {
        let work = if let Some(ref c) = cache {
            match c.get::<crossref_lib::WorkMeta>(doi)? {
                Some(cached) => {
                    if verbose {
                        eprintln!("[verbose] cache HIT for {doi}");
                    }
                    cached
                }
                None => {
                    if verbose {
                        eprintln!("[verbose] cache MISS for {doi}");
                    }
                    let w = client.fetch_work(doi).await?;
                    let _ = c.set(doi, &w);
                    w
                }
            }
        } else {
            client.fetch_work(doi).await?
        };
        records.push(work_to_bib_record_with_style(&work, &key_style));
    }

    if let Some(ref path) = append {
        append_to_bib_file(path, &records)?;
        eprintln!("Appended {} entries to {}", records.len(), path.display());
    } else {
        match format {
            "bibtex" | "table" => {
                println!("{}", records_to_bibtex(&records)?);
            }
            "json" => {
                println!("{}", serde_json::to_string_pretty(&records)?);
            }
            "yaml" => {
                println!("{}", serde_yaml::to_string(&records)
                    .map_err(|e| crossref_lib::CrossrefError::Api(e.to_string()))?);
            }
            _ => {
                println!("{}", records_to_bibtex(&records)?);
            }
        }
    }
    Ok(())
}

async fn cmd_search(
    format: &str,
    no_cache: bool,
    verbose: bool,
    cfg: Arc<Config>,
    query: &crossref_lib::SearchQuery,
) -> crossref_lib::Result<()> {
    let client = CrossrefClient::new(cfg.clone())?;

    // Build a stable cache key from the serialised query
    let cache_key = format!("search:{}", serde_json::to_string(query)?);
    let cache = if no_cache { None } else { DiskCache::from_config(&cfg).ok() };

    if verbose {
        eprintln!("[verbose] cache: {}", if cache.is_some() { "enabled" } else { "disabled" });
    }

    let results = if let Some(ref c) = cache {
        match c.get::<crossref_lib::SearchResult>(&cache_key)? {
            Some(cached) => {
                if verbose {
                    eprintln!("[verbose] cache HIT for search query");
                }
                cached
            }
            None => {
                if verbose {
                    eprintln!("[verbose] cache MISS for search query");
                }
                let r = client.search(query).await?;
                let _ = c.set(&cache_key, &r);
                r
            }
        }
    } else {
        client.search(query).await?
    };

    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&results)?),
        "yaml" => {
            println!("{}", serde_yaml::to_string(&results)
                .map_err(|e| crossref_lib::CrossrefError::Api(e.to_string()))?);
        }
        "fzf" => {
            for item in &results.items {
                let authors = if item.authors.is_empty() {
                    "-".to_string()
                } else {
                    item.authors.join("; ")
                };
                let year = item.year.map(|y| y.to_string()).unwrap_or_else(|| "-".to_string());
                let title = item.title.as_deref().unwrap_or("-");
                let journal = item.journal.as_deref().unwrap_or("-");
                let oa = match item.is_oa {
                    Some(true) => "OA",
                    Some(false) => "",
                    None => "",
                };
                println!("{}\t{}\t{}\t{}\t{}\t{}", item.doi, title, authors, year, journal, oa);
            }
        }
        _ => {
            println!("Found {} results (showing {}):", results.total_results, results.items.len());
            let mut table = Table::new();
            table.set_header(vec!["DOI", "Title", "Authors", "Year", "Journal", "OA"]);
            for item in &results.items {
                table.add_row(vec![
                    Cell::new(&item.doi),
                    Cell::new(item.title.as_deref().unwrap_or("-")),
                    Cell::new(if item.authors.is_empty() {
                        "-".to_string()
                    } else {
                        item.authors.join("; ")
                    }),
                    Cell::new(item.year.map(|y| y.to_string()).unwrap_or_else(|| "-".to_string())),
                    Cell::new(item.journal.as_deref().unwrap_or("-")),
                    Cell::new(match item.is_oa {
                        Some(true) => "Yes",
                        Some(false) => "No",
                        None => "?",
                    }),
                ]);
            }
            println!("{table}");
        }
    }
    Ok(())
}

async fn cmd_pdf(
    cfg: Arc<Config>,
    doi: &str,
    dest_dir: &std::path::Path,
) -> crossref_lib::Result<()> {
    let client = CrossrefClient::new(cfg)?;
    let dest = client.download_pdf(doi, dest_dir).await?;
    println!("Downloaded: {}", dest.display());
    Ok(())
}

fn cmd_config(
    action: ConfigAction,
    config_path: Option<&str>,
) -> crossref_lib::Result<()> {
    match action {
        ConfigAction::Path => {
            let path = crossref_lib::config::resolve_config_path(config_path)?;
            println!("{}", path.display());
        }
        ConfigAction::Show => {
            let cfg = Config::load(None, config_path)?;
            println!("{}", serde_json::to_string_pretty(&cfg)?);
        }
    }
    Ok(())
}

fn cmd_cache(action: CacheAction, cfg: &Config) -> crossref_lib::Result<()> {
    let cache = DiskCache::from_config(cfg)?;
    match action {
        CacheAction::Prune => {
            cache.clear_expired()?;
            eprintln!("Expired cache entries removed.");
        }
        CacheAction::Clear => {
            cache.clear_all()?;
            eprintln!("Cache cleared.");
        }
    }
    Ok(())
}

// ─── Output formatting ────────────────────────────────────────────────────────

fn print_work(work: &crossref_lib::WorkMeta, format: &str) {
    match format {
        "json" => {
            if let Ok(s) = serde_json::to_string_pretty(work) {
                println!("{s}");
            }
        }
        "yaml" => {
            if let Ok(s) = serde_yaml::to_string(work) {
                print!("{s}");
            }
        }
        "bibtex" => {
            let record = work_to_bib_record(work);
            if let Ok(s) = records_to_bibtex(&[record]) {
                print!("{s}");
            }
        }
        "fzf" => {
            let authors = if work.authors.is_empty() {
                "-".to_string()
            } else {
                work.authors.join("; ")
            };
            let year = work.year.map(|y| y.to_string()).unwrap_or_else(|| "-".to_string());
            let title = work.title.as_deref().unwrap_or("-");
            let journal = work.journal.as_deref().unwrap_or("-");
            let oa = match work.is_oa {
                Some(true) => "OA",
                Some(false) => "",
                None => "",
            };
            println!("{}\t{}\t{}\t{}\t{}\t{}", work.doi, title, authors, year, journal, oa);
        }
        _ => {
            // comfy-table output
            let mut table = Table::new();
            table.set_header(vec!["Field", "Value"]);
            table.add_row(vec![Cell::new("DOI"), Cell::new(&work.doi)]);
            if let Some(ref t) = work.title {
                table.add_row(vec![Cell::new("Title"), Cell::new(t)]);
            }
            if !work.authors.is_empty() {
                table.add_row(vec![Cell::new("Authors"), Cell::new(work.authors.join("; "))]);
            }
            if let Some(year) = work.year {
                table.add_row(vec![Cell::new("Year"), Cell::new(year.to_string())]);
            }
            if let Some(ref j) = work.journal {
                table.add_row(vec![Cell::new("Journal"), Cell::new(j)]);
            }
            table.add_row(vec![
                Cell::new("OA"),
                Cell::new(match work.is_oa {
                    Some(true) => "Yes",
                    Some(false) => "No",
                    None => "?",
                }),
            ]);
            if let Some(ref status) = work.oa_status {
                table.add_row(vec![Cell::new("OA Status"), Cell::new(status)]);
            }
            if let Some(ref url) = work.pdf_url {
                table.add_row(vec![Cell::new("PDF"), Cell::new(url)]);
            }
            println!("{table}");
            println!();
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn parse_key_style(s: &str) -> KeyStyle {
    match s {
        "short-title" => KeyStyle::ShortTitle,
        _ => KeyStyle::AuthorYear,
    }
}
