use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::{CrossrefError, Result};

/// A single cached entry wrapping a serializable value with an expiry timestamp.
#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry<T> {
    value: T,
    cached_at: DateTime<Utc>,
}

/// Disk-backed JSON cache for API responses.
pub struct DiskCache {
    dir: PathBuf,
    ttl_days: u32,
}

impl DiskCache {
    /// Construct a `DiskCache` from the resolved [`Config`].
    pub fn from_config(config: &Config) -> Result<Self> {
        let dir = if let Some(ref custom) = config.cache_dir {
            PathBuf::from(custom)
        } else {
            dirs::cache_dir()
                .ok_or_else(|| CrossrefError::Cache("cannot determine cache directory".to_string()))?
                .join("crossref-rs")
        };
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir, ttl_days: config.cache_ttl_days })
    }

    /// Sanitise a cache key into a safe filesystem filename.
    fn key_to_path(&self, key: &str) -> PathBuf {
        let safe: String = key
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
            .collect();
        self.dir.join(format!("{}.json", safe))
    }

    /// Retrieve a cached value for `key` if it exists and has not expired.
    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        if self.ttl_days == 0 {
            return Ok(None);
        }
        let path = self.key_to_path(key);
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&path)?;
        let entry: CacheEntry<T> = serde_json::from_str(&raw)?;

        let age_days = Utc::now()
            .signed_duration_since(entry.cached_at)
            .num_days();
        if age_days > self.ttl_days as i64 {
            let _ = std::fs::remove_file(&path);
            return Ok(None);
        }
        Ok(Some(entry.value))
    }

    /// Store `value` in the cache under `key`.
    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        if self.ttl_days == 0 {
            return Ok(());
        }
        let entry = CacheEntry { value, cached_at: Utc::now() };
        let path = self.key_to_path(key);
        let raw = serde_json::to_string(&entry)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    /// Remove all expired cache entries.
    pub fn clear_expired(&self) -> Result<()> {
        for entry in walkdir::WalkDir::new(&self.dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            // Re-use get() logic: deserialise into raw JSON Value, check timestamp
            let path = entry.path();
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(cached_at_str) = v.get("cached_at").and_then(|v| v.as_str()) {
                        if let Ok(cached_at) = cached_at_str.parse::<DateTime<Utc>>() {
                            let age_days = Utc::now()
                                .signed_duration_since(cached_at)
                                .num_days();
                            if age_days > self.ttl_days as i64 {
                                let _ = std::fs::remove_file(path);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Delete every file in the cache directory.
    pub fn clear_all(&self) -> Result<()> {
        for entry in walkdir::WalkDir::new(&self.dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            std::fs::remove_file(entry.path())?;
        }
        Ok(())
    }
}
