use std::path::PathBuf;

use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::error::{CrossrefError, Result};

/// Runtime configuration resolved from (in priority order):
/// 1. CLI flags / function arguments
/// 2. Environment variables (`CROSSREF_EMAIL`, `CROSSREF_PROXY`, …)
/// 3. Config file (`~/.config/crossref.toml` or `--config` path)
/// 4. Built-in defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Email for Crossref polite-pool access (required for well-behaved usage).
    pub email: Option<String>,
    /// EZproxy host, e.g. `doi-org.ezproxy.cityu.edu.hk`.
    pub proxy: Option<String>,
    /// Default number of rows returned by `crossref search`.
    pub default_rows: u32,
    /// Cache time-to-live in days.
    pub cache_ttl_days: u32,
    /// Override the default XDG cache directory.
    pub cache_dir: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            email: None,
            // 默认代理为香港城市大学 EZproxy
            proxy: Some("doi-org.ezproxy.cityu.edu.hk".to_string()),
            default_rows: 10,
            cache_ttl_days: 30,
            cache_dir: None,
        }
    }
}

impl Config {
    /// Load configuration by merging (CLI override > env vars > config file > defaults).
    /// `email_override` and `config_path` come from CLI flags.
    pub fn load(email_override: Option<&str>, config_path: Option<&str>) -> Result<Self> {
        let path = resolve_config_path(config_path)?;

        let mut cfg: Config = if path.exists() {
            confy::load_path(&path).map_err(|e| CrossrefError::Config(e.to_string()))?
        } else {
            Config::default()
        };

        // Environment variables override file values
        if let Ok(email) = std::env::var("CROSSREF_EMAIL") {
            if !email.is_empty() {
                cfg.email = Some(email);
            }
        }
        if let Ok(proxy) = std::env::var("CROSSREF_PROXY") {
            if !proxy.is_empty() {
                cfg.proxy = Some(proxy);
            }
        }
        if let Ok(rows) = std::env::var("CROSSREF_ROWS") {
            if let Ok(n) = rows.parse::<u32>() {
                cfg.default_rows = n;
            }
        }
        if let Ok(ttl) = std::env::var("CROSSREF_CACHE_TTL_DAYS") {
            if let Ok(n) = ttl.parse::<u32>() {
                cfg.cache_ttl_days = n;
            }
        }

        // CLI flag is highest priority
        if let Some(email) = email_override {
            cfg.email = Some(email.to_string());
        }

        Ok(cfg)
    }

    /// Like [`Config::load`] but also handles first-run guidance for the CLI binary.
    /// Returns `None` when guidance was printed and the caller should exit.
    pub fn load_with_guidance(
        email_override: Option<&str>,
        config_path: Option<&str>,
    ) -> Result<Option<Self>> {
        let path = resolve_config_path(config_path)?;

        // Check whether a usable email is available before loading the full config
        let email_from_env = std::env::var("CROSSREF_EMAIL")
            .ok()
            .filter(|s| !s.is_empty());
        let has_email = email_override.is_some() || email_from_env.is_some();

        if !has_email && !path.exists() {
            create_default_config(&path)?;
            print_first_run_guidance(&path);
            return Ok(None);
        }

        let cfg = Self::load(email_override, config_path)?;

        if cfg.email.as_deref().map(|e| e.is_empty()).unwrap_or(true)
            && !has_email
        {
            print_first_run_guidance(&path);
            return Ok(None);
        }

        Ok(Some(cfg))
    }

    /// Returns `true` when a polite email address is configured.
    pub fn has_email(&self) -> bool {
        self.email.as_deref().map(|e| !e.is_empty()).unwrap_or(false)
    }
}

/// Returns the resolved path to the config file.
pub fn resolve_config_path(override_path: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = override_path {
        return Ok(PathBuf::from(p));
    }
    let dir = dirs::config_dir()
        .ok_or_else(|| CrossrefError::Config("cannot determine config directory".to_string()))?;
    Ok(dir.join("crossref.toml"))
}

/// Write a default config file with bilingual comments to `path`.
pub fn create_default_config(path: &PathBuf) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let now = chrono::Local::now().format("%Y-%m-%d").to_string();
    let content = format!(
        r#"# crossref-rs Default Configuration File
# Auto-generated on {date}
# 自动生成于 {date}

# [REQUIRED] Email for Crossref API polite-pool access.
# 请填写您的真实邮箱地址，以避免被 Crossref 限速。
# Replace with your real email to avoid rate limiting.
email = "your.name@example.com"

# EZproxy host (commonly used by users in Hong Kong / CityU).
# 香港城市大学 EZproxy 地址（如不需要可留空或注释掉）。
proxy = "doi-org.ezproxy.cityu.edu.hk"

# Default number of search results returned per query.
# 搜索时默认返回的结果数量。
default_rows = 10

# Cache expiration in days (0 to disable caching).
# 缓存过期天数（设置为 0 可禁用缓存）。
cache_ttl_days = 30

# Optional: custom cache directory path.
# 可选：自定义缓存目录路径。
# cache_dir = "/path/to/cache"
"#,
        date = now
    );

    std::fs::write(path, content)?;
    Ok(())
}

/// Print the first-run guidance box to stderr.
pub fn print_first_run_guidance(path: &PathBuf) {
    let path_str = path.display().to_string();
    let width = 64usize;
    let inner = width - 2;

    let top    = format!("╔{}╗", "═".repeat(width));
    let sep    = format!("╟{}╢", "─".repeat(width));
    let bot    = format!("╚{}╝", "═".repeat(width));

    let title  = center_pad("crossref-rs First-Run Setup", inner);
    let blank  = pad("", inner);

    let line1  = pad("A default configuration file has been created for you at:", inner);
    let line2  = pad(&format!("  {}", path_str), inner);
    let line3  = pad("Please open it now and set your email address:", inner);
    let line4  = pad(r#"  email = "your.real.email@example.com""#, inner);
    let line5  = pad("Alternatively, set via environment variable (quick setup):", inner);
    let line6  = pad("  • Bash / Zsh   : export CROSSREF_EMAIL=you@example.com", inner);
    let line7  = pad("  • Fish         : set -gx CROSSREF_EMAIL you@example.com", inner);
    let line8  = pad(r#"  • Nushell      : $env.CROSSREF_EMAIL = "you@example.com""#, inner);
    let line9  = pad(r#"  • PowerShell   : $env:CROSSREF_EMAIL = "you@example.com""#, inner);
    let line10 = pad("After editing, re-run your command.", inner);

    let box_str = [
        top.bright_cyan().to_string(),
        row(&title),
        row(&sep),
        row(&blank),
        row(&line1),
        row(&line2),
        row(&blank),
        row(&line3),
        row(&line4),
        row(&blank),
        row(&line5),
        row(&line6),
        row(&line7),
        row(&line8),
        row(&line9),
        row(&blank),
        row(&line10),
        row(&blank),
        bot.bright_cyan().to_string(),
    ]
    .join("\n");

    eprintln!("{}", box_str);
}

fn pad(s: &str, width: usize) -> String {
    format!("{:<width$}", s, width = width)
}

fn center_pad(s: &str, width: usize) -> String {
    let len = s.len();
    if len >= width {
        return s.to_string();
    }
    let total_pad = width - len;
    let left = total_pad / 2;
    let right = total_pad - left;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(right))
}

fn row(content: &str) -> String {
    format!("║{}║", content)
}
