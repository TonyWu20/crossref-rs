use std::env;
use std::sync::Mutex;
use tempfile::NamedTempFile;

use crossref_lib::config::{Config, create_default_config, resolve_config_path};

/// Serialize all env-var tests to prevent races across test threads.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// A RAII guard that clears an env var on drop (for test isolation).
struct EnvVarGuard {
    key: &'static str,
    original: Option<String>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let original = env::var(key).ok();
        env::set_var(key, value);
        Self { key, original }
    }

    fn remove(key: &'static str) -> Self {
        let original = env::var(key).ok();
        env::remove_var(key);
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(v) => env::set_var(self.key, v),
            None => env::remove_var(self.key),
        }
    }
}

#[test]
fn test_load_env_overrides_file() {
    let _lock = ENV_MUTEX.lock().unwrap();
    // Create a temp config file with one email
    let file = NamedTempFile::with_suffix(".toml").unwrap();
    let toml = r#"email = "file@example.com"
proxy = ""
default_rows = 10
cache_ttl_days = 30
"#;
    std::fs::write(file.path(), toml).unwrap();

    // Set env var to a different email
    let _guard = EnvVarGuard::set("CROSSREF_EMAIL", "env@example.com");
    // Clear any CLI override possibility by using None
    let cfg = Config::load(None, Some(file.path().to_str().unwrap())).unwrap();

    assert_eq!(
        cfg.email.as_deref(),
        Some("env@example.com"),
        "environment variable should override config file"
    );
}

#[test]
fn test_load_cli_overrides_env() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let file = NamedTempFile::with_suffix(".toml").unwrap();
    let toml = r#"email = "file@example.com"
proxy = ""
default_rows = 10
cache_ttl_days = 30
"#;
    std::fs::write(file.path(), toml).unwrap();

    // Set env var
    let _guard = EnvVarGuard::set("CROSSREF_EMAIL", "env@example.com");

    // Pass CLI override
    let cfg = Config::load(
        Some("cli@example.com"),
        Some(file.path().to_str().unwrap()),
    )
    .unwrap();

    assert_eq!(
        cfg.email.as_deref(),
        Some("cli@example.com"),
        "CLI override should have highest priority"
    );
}

#[test]
fn test_load_with_guidance_returns_none_when_no_email() {
    let _lock = ENV_MUTEX.lock().unwrap();
    // Use a temp path that doesn't exist yet so the first-run path is triggered
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("crossref_test.toml").to_string_lossy().to_string();

    // Ensure no email from env
    let _guard = EnvVarGuard::remove("CROSSREF_EMAIL");

    let result = Config::load_with_guidance(None, Some(&config_path)).unwrap();
    assert!(
        result.is_none(),
        "load_with_guidance should return None when no email is configured"
    );
    // Config file should have been created
    assert!(
        std::path::Path::new(&config_path).exists(),
        "default config file should be created on first run"
    );
}

#[test]
fn test_load_with_guidance_succeeds_with_email() {
    let _lock = ENV_MUTEX.lock().unwrap();
    let file = NamedTempFile::with_suffix(".toml").unwrap();
    let toml = r#"email = "user@example.com"
proxy = ""
default_rows = 10
cache_ttl_days = 30
"#;
    std::fs::write(file.path(), toml).unwrap();

    let _guard = EnvVarGuard::remove("CROSSREF_EMAIL");

    let result = Config::load_with_guidance(
        None,
        Some(file.path().to_str().unwrap()),
    )
    .unwrap();

    assert!(result.is_some(), "should succeed when email is set in config");
    assert_eq!(result.unwrap().email.as_deref(), Some("user@example.com"));
}

#[test]
fn test_create_default_config_creates_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crossref.toml");
    create_default_config(&path).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("email"), "default config should contain email field");
    assert!(content.contains("cache_ttl_days"), "default config should contain cache_ttl_days");
}

#[test]
fn test_resolve_config_path_uses_default() {
    let path = resolve_config_path(None).unwrap();
    assert!(
        path.ends_with("crossref.toml"),
        "default path should end with crossref.toml"
    );
}

#[test]
fn test_resolve_config_path_with_override() {
    let path = resolve_config_path(Some("/tmp/my_config.toml")).unwrap();
    assert_eq!(path.to_str().unwrap(), "/tmp/my_config.toml");
}

#[test]
fn test_has_email_true() {
    let cfg = Config {
        email: Some("test@example.com".to_string()),
        ..Config::default()
    };
    assert!(cfg.has_email());
}

#[test]
fn test_has_email_false_when_empty() {
    let cfg = Config {
        email: Some(String::new()),
        ..Config::default()
    };
    assert!(!cfg.has_email());
}

#[test]
fn test_has_email_false_when_none() {
    let cfg = Config {
        email: None,
        ..Config::default()
    };
    assert!(!cfg.has_email());
}
