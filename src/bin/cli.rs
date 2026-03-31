use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};

use crossref_lib::{
    bibtex::{append_to_bib_file, records_to_bibtex, work_to_bib_record},
    cache::DiskCache,
    client::CrossrefClient,
    config::Config,
    models::SearchQueryBuilder,
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
    #[arg(long, global = true, value_name = "FILE")]
    config: Option<String>,

    /// Override polite email for this invocation
    #[arg(long, global = true, env = "CROSSREF_EMAIL", value_name = "EMAIL")]
    email: Option<String>,

    /// Disable response caching
    #[arg(long, global = true)]
    no_cache: bool,

    /// Output format
    #[arg(long, short = 'f', global = true, default_value = "table",
          value_parser = ["table", "json", "bibtex", "yaml"])]
    format: String,

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
        #[arg(long, value_name = "FILE")]
        append: Option<PathBuf>,

        /// Citation key style
        #[arg(long, default_value = "author-year",
              value_parser = ["author-year", "short-title"])]
        key_style: String,
    },

    /// Search Crossref for literature
    Search {
        /// Free-text query
        #[arg(value_name = "QUERY")]
        query: Option<String>,

        /// Filter by title
        #[arg(long)]
        title: Option<String>,

        /// Filter by author
        #[arg(long)]
        author: Option<String>,

        /// Earliest publication year
        #[arg(long, value_name = "YEAR")]
        year_from: Option<i32>,

        /// Latest publication year
        #[arg(long, value_name = "YEAR")]
        year_to: Option<i32>,

        /// Work type filter (e.g. journal-article)
        #[arg(long, value_name = "TYPE")]
        r#type: Option<String>,

        /// Only return open-access items
        #[arg(long)]
        open_access: bool,

        /// Number of results to return
        #[arg(long, short = 'n', default_value_t = 10)]
        rows: u32,

        /// Sort order (score, updated, deposited, indexed, published)
        #[arg(long, default_value = "score")]
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
        eprintln!("error: {}", e);
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

    match cli.command {
        Commands::FetchMeta { dois } => {
            cmd_fetch_meta(&cli.format, cli.no_cache, cfg, &dois).await?;
        }
        Commands::FetchBib { dois, append, key_style: _ } => {
            cmd_fetch_bib(&cli.format, cli.no_cache, cfg, &dois, append).await?;
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
            cmd_search(&cli.format, cli.no_cache, cfg, &search_q).await?;
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
    cfg: Arc<Config>,
    dois: &[String],
) -> crossref_lib::Result<()> {
    let client = CrossrefClient::new(cfg.clone())?;
    let cache = if no_cache { None } else { DiskCache::from_config(&cfg).ok() };

    for doi in dois {
        let work = if let Some(ref c) = cache {
            match c.get::<crossref_lib::WorkMeta>(doi)? {
                Some(cached) => cached,
                None => {
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
    cfg: Arc<Config>,
    dois: &[String],
    append: Option<PathBuf>,
) -> crossref_lib::Result<()> {
    let client = CrossrefClient::new(cfg.clone())?;
    let cache = if no_cache { None } else { DiskCache::from_config(&cfg).ok() };

    let mut records = Vec::new();
    for doi in dois {
        let work = if let Some(ref c) = cache {
            match c.get::<crossref_lib::WorkMeta>(doi)? {
                Some(cached) => cached,
                None => {
                    let w = client.fetch_work(doi).await?;
                    let _ = c.set(doi, &w);
                    w
                }
            }
        } else {
            client.fetch_work(doi).await?
        };
        records.push(work_to_bib_record(&work));
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
            _ => {
                println!("{}", records_to_bibtex(&records)?);
            }
        }
    }
    Ok(())
}

async fn cmd_search(
    format: &str,
    _no_cache: bool,
    cfg: Arc<Config>,
    query: &crossref_lib::SearchQuery,
) -> crossref_lib::Result<()> {
    let client = CrossrefClient::new(cfg)?;
    let results = client.search(query).await?;
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&results)?),
        _ => {
            println!("Found {} results (showing {}):", results.total_results, results.items.len());
            for item in &results.items {
                print_work(item, "table");
            }
        }
    }
    Ok(())
}

async fn cmd_pdf(
    cfg: Arc<Config>,
    doi: &str,
    dest_dir: &PathBuf,
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
                println!("{}", s);
            }
        }
        "bibtex" => {
            let record = work_to_bib_record(work);
            if let Ok(s) = records_to_bibtex(&[record]) {
                print!("{}", s);
            }
        }
        _ => {
            // Simple table-like output
            println!("DOI:       {}", work.doi);
            if let Some(ref t) = work.title {
                println!("Title:     {}", t);
            }
            if !work.authors.is_empty() {
                println!("Authors:   {}", work.authors.join("; "));
            }
            if let Some(year) = work.year {
                println!("Year:      {}", year);
            }
            if let Some(ref j) = work.journal {
                println!("Journal:   {}", j);
            }
            println!();
        }
    }
}
