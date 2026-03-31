use crossref_lib::cache::DiskCache;
use crossref_lib::config::Config;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

/// Helper to create a DiskCache backed by a temp directory.
fn make_cache(ttl_days: u32) -> (DiskCache, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let cache_path = dir.path().to_string_lossy().to_string();
    let cfg = Config {
        cache_ttl_days: ttl_days,
        cache_dir: Some(cache_path),
        ..Config::default()
    };
    let cache = DiskCache::from_config(&cfg).unwrap();
    (cache, dir)
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TestValue {
    name: String,
    count: u32,
}

#[test]
fn test_cache_set_get() {
    let (cache, _dir) = make_cache(30);
    let value = TestValue { name: "hello".to_string(), count: 42 };

    cache.set("key1", &value).unwrap();
    let retrieved: Option<TestValue> = cache.get("key1").unwrap();

    assert_eq!(retrieved, Some(value));
}

#[test]
fn test_cache_miss_for_unknown_key() {
    let (cache, _dir) = make_cache(30);
    let retrieved: Option<TestValue> = cache.get("nonexistent").unwrap();
    assert!(retrieved.is_none());
}

#[test]
fn test_cache_disabled_when_ttl_0() {
    let (cache, _dir) = make_cache(0);
    let value = TestValue { name: "test".to_string(), count: 1 };

    // set should be a no-op
    cache.set("key1", &value).unwrap();
    // get should always return None
    let retrieved: Option<TestValue> = cache.get("key1").unwrap();
    assert!(retrieved.is_none(), "cache should return None when TTL is 0");
}

#[test]
fn test_cache_clear_all() {
    let (cache, _dir) = make_cache(30);
    let v1 = TestValue { name: "a".to_string(), count: 1 };
    let v2 = TestValue { name: "b".to_string(), count: 2 };

    cache.set("key1", &v1).unwrap();
    cache.set("key2", &v2).unwrap();

    cache.clear_all().unwrap();

    let r1: Option<TestValue> = cache.get("key1").unwrap();
    let r2: Option<TestValue> = cache.get("key2").unwrap();
    assert!(r1.is_none(), "key1 should be cleared");
    assert!(r2.is_none(), "key2 should be cleared");
}

#[test]
fn test_cache_overwrite_key() {
    let (cache, _dir) = make_cache(30);
    let v1 = TestValue { name: "first".to_string(), count: 1 };
    let v2 = TestValue { name: "second".to_string(), count: 2 };

    cache.set("key1", &v1).unwrap();
    cache.set("key1", &v2).unwrap();

    let result: Option<TestValue> = cache.get("key1").unwrap();
    assert_eq!(result, Some(v2), "second write should overwrite first");
}

#[test]
fn test_cache_key_sanitisation() {
    let (cache, _dir) = make_cache(30);
    let value = TestValue { name: "sanitised".to_string(), count: 0 };

    // Keys with special characters should be sanitised to safe filenames
    cache.set("10.1234/some:doi?extra=chars", &value).unwrap();
    let result: Option<TestValue> = cache.get("10.1234/some:doi?extra=chars").unwrap();
    assert_eq!(result, Some(value));
}

#[test]
fn test_cache_clear_expired_removes_old_entries() {
    // We can't easily simulate TTL expiry without mocking time, but we can
    // verify that clear_expired runs without error on a fresh cache.
    let (cache, _dir) = make_cache(1);
    let value = TestValue { name: "fresh".to_string(), count: 99 };
    cache.set("new_key", &value).unwrap();
    // clear_expired on a fresh cache should not remove valid entries
    cache.clear_expired().unwrap();
    let result: Option<TestValue> = cache.get("new_key").unwrap();
    assert_eq!(result, Some(value), "fresh entry should survive clear_expired");
}
