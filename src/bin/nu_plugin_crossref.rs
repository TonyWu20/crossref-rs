use nu_plugin::{
    serve_plugin, EvaluatedCall, EngineInterface, MsgPackSerializer, Plugin, PluginCommand,
    SimplePluginCommand,
};
use nu_protocol::{LabeledError, Signature, SyntaxShape, Value};

use crossref_lib::{
    bibtex::{records_to_bibtex, work_to_bib_record},
    client::CrossrefClient,
    config::Config,
    models::SearchQueryBuilder,
};

// ─── Plugin root ──────────────────────────────────────────────────────────────

struct CrossrefPlugin;

impl Plugin for CrossrefPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![
            Box::new(FetchMetaCommand),
            Box::new(FetchBibCommand),
            Box::new(SearchCommand),
            Box::new(PdfCommand),
        ]
    }
}

// ─── crossref fetch-meta ──────────────────────────────────────────────────────

struct FetchMetaCommand;

impl SimplePluginCommand for FetchMetaCommand {
    type Plugin = CrossrefPlugin;

    fn name(&self) -> &str {
        "crossref fetch-meta"
    }

    fn description(&self) -> &str {
        "Fetch literature metadata for one or more DOIs"
    }

    fn signature(&self) -> Signature {
        Signature::build("crossref fetch-meta")
            .required("doi", SyntaxShape::String, "DOI to look up")
            .named("email", SyntaxShape::String, "Override polite email", Some('e'))
    }

    fn run(
        &self,
        _plugin: &CrossrefPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let doi: String = call.req(0)?;
        let email: Option<String> = call.get_flag("email")?;

        let cfg = load_config(email.as_deref(), engine)?;
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| LabeledError::new(e.to_string()))?;
        let client = CrossrefClient::new(std::sync::Arc::new(cfg))
            .map_err(|e| LabeledError::new(e.to_string()))?;

        let work = rt
            .block_on(client.fetch_work(&doi))
            .map_err(|e| LabeledError::new(e.to_string()))?;

        Ok(work_meta_to_nu_value(&work, call.head))
    }
}

// ─── crossref fetch-bib ───────────────────────────────────────────────────────

struct FetchBibCommand;

impl SimplePluginCommand for FetchBibCommand {
    type Plugin = CrossrefPlugin;

    fn name(&self) -> &str {
        "crossref fetch-bib"
    }

    fn description(&self) -> &str {
        "Fetch a BibTeX entry for one or more DOIs"
    }

    fn signature(&self) -> Signature {
        Signature::build("crossref fetch-bib")
            .required("doi", SyntaxShape::String, "DOI to look up")
            .named("email", SyntaxShape::String, "Override polite email", Some('e'))
    }

    fn run(
        &self,
        _plugin: &CrossrefPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let doi: String = call.req(0)?;
        let email: Option<String> = call.get_flag("email")?;

        let cfg = load_config(email.as_deref(), engine)?;
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| LabeledError::new(e.to_string()))?;
        let client = CrossrefClient::new(std::sync::Arc::new(cfg))
            .map_err(|e| LabeledError::new(e.to_string()))?;

        let work = rt
            .block_on(client.fetch_work(&doi))
            .map_err(|e| LabeledError::new(e.to_string()))?;

        let record = work_to_bib_record(&work);
        let bibtex = records_to_bibtex(&[record])
            .map_err(|e| LabeledError::new(e.to_string()))?;

        Ok(Value::string(bibtex, call.head))
    }
}

// ─── crossref search ──────────────────────────────────────────────────────────

struct SearchCommand;

impl SimplePluginCommand for SearchCommand {
    type Plugin = CrossrefPlugin;

    fn name(&self) -> &str {
        "crossref search"
    }

    fn description(&self) -> &str {
        "Search Crossref for literature"
    }

    fn signature(&self) -> Signature {
        Signature::build("crossref search")
            .optional("query", SyntaxShape::String, "Free-text query")
            .named("title", SyntaxShape::String, "Filter by title", Some('t'))
            .named("author", SyntaxShape::String, "Filter by author", Some('a'))
            .named("rows", SyntaxShape::Int, "Number of results", Some('n'))
            .named("email", SyntaxShape::String, "Override polite email", Some('e'))
            .named("year-from", SyntaxShape::Int, "Earliest publication year", None)
            .named("year-to", SyntaxShape::Int, "Latest publication year", None)
            .named("type", SyntaxShape::String, "Work type filter (e.g. journal-article)", None)
            .named("open-access", SyntaxShape::Boolean, "Only return open-access items", None)
            .named("sort", SyntaxShape::String, "Sort order (score, updated, deposited, indexed, published)", Some('s'))
    }

    fn run(
        &self,
        _plugin: &CrossrefPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let query: Option<String> = call.opt(0)?;
        let title: Option<String> = call.get_flag("title")?;
        let author: Option<String> = call.get_flag("author")?;
        let rows: Option<i64> = call.get_flag("rows")?;
        let email: Option<String> = call.get_flag("email")?;
        let year_from: Option<i64> = call.get_flag("year-from")?;
        let year_to: Option<i64> = call.get_flag("year-to")?;
        let work_type: Option<String> = call.get_flag("type")?;
        let open_access: Option<bool> = call.get_flag("open-access")?;
        let sort: Option<String> = call.get_flag("sort")?;

        let search_q = SearchQueryBuilder::default()
            .query(query)
            .title(title)
            .author(author)
            .rows(rows.map(|r| r as u32).unwrap_or(10))
            .year_from(year_from.map(|y| y as i32))
            .year_to(year_to.map(|y| y as i32))
            .work_type(work_type)
            .open_access(open_access.unwrap_or(false))
            .sort(sort)
            .build()
            .map_err(|e| LabeledError::new(e.to_string()))?;

        let cfg = load_config(email.as_deref(), engine)?;
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| LabeledError::new(e.to_string()))?;
        let client = CrossrefClient::new(std::sync::Arc::new(cfg))
            .map_err(|e| LabeledError::new(e.to_string()))?;

        let results = rt
            .block_on(client.search(&search_q))
            .map_err(|e| LabeledError::new(e.to_string()))?;

        let list: Vec<Value> = results
            .items
            .iter()
            .map(|w| work_meta_to_nu_value(w, call.head))
            .collect();

        Ok(Value::list(list, call.head))
    }
}

// ─── crossref pdf ─────────────────────────────────────────────────────────────

struct PdfCommand;

impl SimplePluginCommand for PdfCommand {
    type Plugin = CrossrefPlugin;

    fn name(&self) -> &str {
        "crossref pdf"
    }

    fn description(&self) -> &str {
        "Download the best available OA PDF for a DOI"
    }

    fn signature(&self) -> Signature {
        Signature::build("crossref pdf")
            .required("doi", SyntaxShape::String, "DOI to look up")
            .named("output", SyntaxShape::Filepath, "Output directory", Some('o'))
            .named("email", SyntaxShape::String, "Override polite email", Some('e'))
    }

    fn run(
        &self,
        _plugin: &CrossrefPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: &Value,
    ) -> Result<Value, LabeledError> {
        let doi: String = call.req(0)?;
        let output: Option<String> = call.get_flag("output")?;
        let email: Option<String> = call.get_flag("email")?;

        let dest_dir = output
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let cfg = load_config(email.as_deref(), engine)?;
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| LabeledError::new(e.to_string()))?;
        let client = CrossrefClient::new(std::sync::Arc::new(cfg))
            .map_err(|e| LabeledError::new(e.to_string()))?;

        let path = rt
            .block_on(client.download_pdf(&doi, &dest_dir))
            .map_err(|e| LabeledError::new(e.to_string()))?;

        Ok(Value::string(path.display().to_string(), call.head))
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Load config for the plugin context (no first-run guidance).
///
/// Returns an error with setup instructions if no email is configured.
/// Set the `CROSSREF_EMAIL` environment variable or add `email = "..."` to
/// your config file (`~/.config/crossref.toml`).
fn load_config(
    email_override: Option<&str>,
    _engine: &EngineInterface,
) -> Result<Config, LabeledError> {
    let cfg = Config::load(email_override, None)
        .map_err(|e| LabeledError::new(e.to_string()))?;

    if !cfg.has_email() {
        return Err(LabeledError::new(
            "No email address configured for Crossref polite-pool access. \
             Set CROSSREF_EMAIL env var or add `email = \"you@example.com\"` \
             to ~/.config/crossref.toml."
                .to_string(),
        ));
    }

    Ok(cfg)
}

/// Convert a `WorkMeta` to a Nushell `Value::Record`.
fn work_meta_to_nu_value(work: &crossref_lib::WorkMeta, span: nu_protocol::Span) -> Value {
    use nu_protocol::Record;

    let mut record = Record::new();
    record.push("doi", Value::string(work.doi.clone(), span));
    record.push(
        "title",
        work.title
            .as_deref()
            .map(|t| Value::string(t, span))
            .unwrap_or(Value::nothing(span)),
    );
    record.push(
        "authors",
        Value::list(
            work.authors.iter().map(|a| Value::string(a, span)).collect(),
            span,
        ),
    );
    record.push(
        "year",
        work.year
            .map(|y| Value::int(y as i64, span))
            .unwrap_or(Value::nothing(span)),
    );
    record.push(
        "journal",
        work.journal
            .as_deref()
            .map(|j| Value::string(j, span))
            .unwrap_or(Value::nothing(span)),
    );
    record.push(
        "is_oa",
        work.is_oa
            .map(|v| Value::bool(v, span))
            .unwrap_or(Value::nothing(span)),
    );
    record.push(
        "oa_status",
        work.oa_status
            .as_deref()
            .map(|s| Value::string(s, span))
            .unwrap_or(Value::nothing(span)),
    );
    record.push(
        "pdf_url",
        work.pdf_url
            .as_deref()
            .map(|u| Value::string(u, span))
            .unwrap_or(Value::nothing(span)),
    );

    Value::record(record, span)
}

// ─── Entry point ──────────────────────────────────────────────────────────────

fn main() {
    serve_plugin(&CrossrefPlugin, MsgPackSerializer)
}
