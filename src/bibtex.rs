use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use serde_bibtex::de::Deserializer;
use serde::Deserialize;

use crate::error::{CrossrefError, Result};
use crate::models::{BibRecord, WorkMeta};
use crate::utils::generate_citation_key;

// ─── Conversion ──────────────────────────────────────────────────────────────

/// Convert a `WorkMeta` into a `BibRecord` suitable for BibTeX serialization.
///
/// The citation key is generated from the author list and publication year;
/// conflict resolution against an existing bibliography must be handled by the
/// caller via [`crate::utils::resolve_key_conflict`].
pub fn work_to_bib_record(work: &WorkMeta) -> BibRecord {
    let entry_key = generate_citation_key(&work.authors, work.year);
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

/// Append `records` to the `.bib` file at `path`, deduplicating by `entry_key`.
///
/// If `path` does not exist it is created.
pub fn append_to_bib_file(path: &Path, records: &[BibRecord]) -> Result<()> {
    // Read existing keys from the file if it exists
    let existing_keys: Vec<String> = if path.exists() {
        let content = std::fs::read_to_string(path)?;
        parse_entry_keys(&content)
    } else {
        Vec::new()
    };

    let new_records: Vec<&BibRecord> = records
        .iter()
        .filter(|r| !existing_keys.contains(&r.entry_key))
        .collect();

    if new_records.is_empty() {
        return Ok(());
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    for record in new_records {
        let bibtex = serde_bibtex::to_string(&[record])
            .map_err(|e| CrossrefError::Bibtex(e.to_string()))?;
        writeln!(file, "{}", bibtex)?;
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
    #[allow(dead_code)]
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
}
