use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use serde_bibtex::de::Deserializer;
use serde::Deserialize;

use crate::error::{CrossrefError, Result};
use crate::models::{BibRecord, WorkMeta};
use crate::utils::{generate_citation_key_by_style, resolve_key_conflict, KeyStyle};

// ─── Conversion ──────────────────────────────────────────────────────────────

/// Convert a `WorkMeta` into a `BibRecord` using `AuthorYear` key style.
///
/// Conflict resolution against an existing bibliography must be handled by the
/// caller via [`crate::utils::resolve_key_conflict`] or by using
/// [`append_to_bib_file`] which resolves conflicts automatically.
pub fn work_to_bib_record(work: &WorkMeta) -> BibRecord {
    work_to_bib_record_with_style(work, &KeyStyle::AuthorYear)
}

/// Convert a `WorkMeta` into a `BibRecord` with the specified key style.
pub fn work_to_bib_record_with_style(work: &WorkMeta, style: &KeyStyle) -> BibRecord {
    let entry_key = generate_citation_key_by_style(work, style);
    let entry_type = work_type_to_bib_type(work.work_type.as_deref());

    let mut fields = BTreeMap::new();

    if let Some(ref title) = work.title {
        fields.insert("title".to_string(), title.clone());
    }
    if !work.authors.is_empty() {
        fields.insert("author".to_string(), work.authors.join(" and "));
    }
    if let Some(year) = work.year {
        fields.insert("year".to_string(), year.to_string());
    }
    if let Some(ref journal) = work.journal {
        fields.insert("journal".to_string(), journal.clone());
    }
    if let Some(ref volume) = work.volume {
        fields.insert("volume".to_string(), volume.clone());
    }
    if let Some(ref issue) = work.issue {
        fields.insert("number".to_string(), issue.clone());
    }
    if let Some(ref pages) = work.pages {
        fields.insert("pages".to_string(), pages.clone());
    }
    if let Some(ref publisher) = work.publisher {
        fields.insert("publisher".to_string(), publisher.clone());
    }
    fields.insert("doi".to_string(), work.doi.clone());

    BibRecord { entry_type, entry_key, fields }
}

/// Serialize a list of `BibRecord`s to a BibTeX string.
pub fn records_to_bibtex(records: &[BibRecord]) -> Result<String> {
    serde_bibtex::to_string(records)
        .map_err(|e| CrossrefError::Bibtex(e.to_string()))
}

// ─── File operations ─────────────────────────────────────────────────────────

/// Append `records` to the `.bib` file at `path`.
///
/// Deduplicates by DOI: if a record's DOI is already present in the file,
/// it is skipped. Within-batch key conflicts (two different records that
/// generate the same base citation key) are resolved with letter suffixes
/// (`a`, `b`, …).
///
/// If `path` does not exist it is created.
pub fn append_to_bib_file(path: &Path, records: &[BibRecord]) -> Result<()> {
    // Read existing keys and DOIs from the file
    let (existing_keys, existing_dois): (Vec<String>, Vec<String>) = if path.exists() {
        let content = std::fs::read_to_string(path)?;
        let de = Deserializer::from_str(&content);
        let existing: Vec<BibKeyRecord> = de
            .into_iter_regular_entry::<BibKeyRecord>()
            .filter_map(|r| r.ok())
            .collect();
        let keys = existing.iter().map(|r| r.entry_key.clone()).collect();
        let dois = existing
            .iter()
            .filter_map(|r| r.fields.get("doi").cloned())
            .collect();
        (keys, dois)
    } else {
        (Vec::new(), Vec::new())
    };

    let mut committed_keys = existing_keys;
    let mut committed_dois = existing_dois;
    let mut new_records: Vec<BibRecord> = Vec::new();

    for record in records {
        // Dedup by DOI: skip if this DOI is already in the file or this batch
        let record_doi = record.fields.get("doi").map(|s| s.as_str()).unwrap_or("");
        if !record_doi.is_empty() && committed_dois.contains(&record_doi.to_string()) {
            continue;
        }

        // Resolve key conflict against existing file keys + already-queued records
        let resolved_key = resolve_key_conflict(&record.entry_key, &committed_keys);
        let mut r = record.clone();
        r.entry_key = resolved_key.clone();
        committed_keys.push(resolved_key);
        if !record_doi.is_empty() {
            committed_dois.push(record_doi.to_string());
        }
        new_records.push(r);
    }

    if new_records.is_empty() {
        return Ok(());
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    for record in &new_records {
        let bibtex = serde_bibtex::to_string(&[record])
            .map_err(|e| CrossrefError::Bibtex(e.to_string()))?;
        writeln!(file, "{bibtex}")?;
    }

    Ok(())
}

// ─── Parsing helpers ─────────────────────────────────────────────────────────

/// A minimal struct used only for reading existing `.bib` files.
#[derive(Debug, Deserialize)]
struct BibKeyRecord {
    #[allow(dead_code)]
    entry_type: String,
    entry_key: String,
    fields: BTreeMap<String, String>,
}

/// Parse the citation keys that are already present in a `.bib` string.
pub fn parse_entry_keys(content: &str) -> Vec<String> {
    let de = Deserializer::from_str(content);
    de.into_iter_regular_entry::<BibKeyRecord>()
        .filter_map(|r| r.ok())
        .map(|r| r.entry_key)
        .collect()
}

// ─── Private helpers ─────────────────────────────────────────────────────────

/// Map a Crossref work type string to a BibTeX entry type.
fn work_type_to_bib_type(work_type: Option<&str>) -> String {
    match work_type {
        Some("journal-article") => "article",
        Some("book") | Some("monograph") => "book",
        Some("book-chapter") => "inbook",
        Some("proceedings-article") => "inproceedings",
        Some("proceedings") => "proceedings",
        Some("dissertation") => "phdthesis",
        Some("report") | Some("report-component") => "techreport",
        Some("posted-content") => "misc",
        _ => "misc",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::WorkMeta;

    fn sample_work() -> WorkMeta {
        WorkMeta {
            doi: "10.1234/test".to_string(),
            title: Some("A Test Article".to_string()),
            authors: vec!["Smith, John".to_string()],
            year: Some(2024),
            journal: Some("Journal of Testing".to_string()),
            work_type: Some("journal-article".to_string()),
            ..WorkMeta::default()
        }
    }

    #[test]
    fn test_work_to_bib_record_fields() {
        let work = sample_work();
        let record = work_to_bib_record(&work);
        assert_eq!(record.entry_type, "article");
        assert_eq!(record.entry_key, "Smith2024");
        assert_eq!(record.fields["title"], "A Test Article");
        assert_eq!(record.fields["doi"], "10.1234/test");
    }

    #[test]
    fn test_parse_entry_keys() {
        let bib = "@article{Smith2024,\n  title = {Test},\n  year = {2024},\n}\n";
        let keys = parse_entry_keys(bib);
        assert_eq!(keys, vec!["Smith2024"]);
    }

    #[test]
    fn test_work_to_bib_record_short_title_style() {
        let work = sample_work();
        let record = work_to_bib_record_with_style(&work, &KeyStyle::ShortTitle);
        // Title: "A Test Article" → stop word "A" removed → "Test", "Article"
        assert_eq!(record.entry_key, "TestArticle2024");
    }

    #[test]
    fn test_append_deduplicates_by_doi() {
        use tempfile::NamedTempFile;
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        let record = work_to_bib_record(&sample_work());

        // First append: adds the record
        append_to_bib_file(path, &[record.clone()]).unwrap();
        let content1 = std::fs::read_to_string(path).unwrap();
        let keys1 = parse_entry_keys(&content1);
        assert_eq!(keys1, vec!["Smith2024"]);

        // Second append of same DOI: should be a no-op
        append_to_bib_file(path, &[record.clone()]).unwrap();
        let content2 = std::fs::read_to_string(path).unwrap();
        let keys2 = parse_entry_keys(&content2);
        assert_eq!(keys2, vec!["Smith2024"], "second append should be idempotent");
    }

    #[test]
    fn test_conflict_suffix_applied() {
        use tempfile::NamedTempFile;
        let file = NamedTempFile::new().unwrap();
        let path = file.path();

        // Two records with the same author+year but different DOIs
        let work_a = WorkMeta {
            doi: "10.1234/a".to_string(),
            title: Some("First Paper".to_string()),
            authors: vec!["Smith, John".to_string()],
            year: Some(2024),
            work_type: Some("journal-article".to_string()),
            ..WorkMeta::default()
        };
        let work_b = WorkMeta {
            doi: "10.1234/b".to_string(),
            title: Some("Second Paper".to_string()),
            authors: vec!["Smith, John".to_string()],
            year: Some(2024),
            work_type: Some("journal-article".to_string()),
            ..WorkMeta::default()
        };

        let record_a = work_to_bib_record(&work_a);
        let record_b = work_to_bib_record(&work_b);
        // Both generate "Smith2024" as base key

        append_to_bib_file(path, &[record_a, record_b]).unwrap();
        let content = std::fs::read_to_string(path).unwrap();
        let keys = parse_entry_keys(&content);

        assert_eq!(keys.len(), 2, "both records should be written");
        assert!(keys.contains(&"Smith2024".to_string()), "first record should have base key");
        assert!(keys.contains(&"Smith2024a".to_string()), "second record should have 'a' suffix");
    }
}
