use crossref_lib::bibtex::{
    append_to_bib_file, parse_entry_keys, records_to_bibtex, work_to_bib_record,
    work_to_bib_record_with_style,
};
use crossref_lib::models::WorkMeta;
use crossref_lib::utils::KeyStyle;
use tempfile::NamedTempFile;

fn make_work(doi: &str, author_family: &str, year: i32, title: &str) -> WorkMeta {
    WorkMeta {
        doi: doi.to_string(),
        title: Some(title.to_string()),
        authors: vec![format!("{}, Test", author_family)],
        year: Some(year),
        work_type: Some("journal-article".to_string()),
        journal: Some("Test Journal".to_string()),
        ..WorkMeta::default()
    }
}

// ─── Citation key style tests ─────────────────────────────────────────────────

#[test]
fn test_author_year_style_produces_correct_key() {
    let work = make_work("10.1/a", "Johnson", 2022, "Some Title");
    let record = work_to_bib_record(&work);
    assert_eq!(record.entry_key, "Johnson2022");
}

#[test]
fn test_short_title_style_produces_correct_key() {
    let work = make_work("10.1/b", "Johnson", 2022, "Deep Learning Methods");
    let record = work_to_bib_record_with_style(&work, &KeyStyle::ShortTitle);
    // "Deep", "Learning", "Methods" (no stop words)
    assert_eq!(record.entry_key, "DeepLearningMethods2022");
}

#[test]
fn test_short_title_strips_stop_words() {
    let work = make_work("10.1/c", "Lee", 2021, "A Study of the Brain");
    let record = work_to_bib_record_with_style(&work, &KeyStyle::ShortTitle);
    // "A" and "of" and "the" are stop words → "Study", "Brain"
    assert_eq!(record.entry_key, "StudyBrain2021");
}

#[test]
fn test_short_title_max_four_words() {
    let work = make_work(
        "10.1/d",
        "Park",
        2020,
        "Word1 Word2 Word3 Word4 Word5 Word6",
    );
    let record = work_to_bib_record_with_style(&work, &KeyStyle::ShortTitle);
    assert_eq!(record.entry_key, "Word1Word2Word3Word42020");
}

#[test]
fn test_short_title_no_title_falls_back_to_unknown() {
    let work = WorkMeta {
        doi: "10.1/e".to_string(),
        title: None,
        year: Some(2019),
        ..WorkMeta::default()
    };
    let record = work_to_bib_record_with_style(&work, &KeyStyle::ShortTitle);
    assert_eq!(record.entry_key, "Unknown2019");
}

// ─── BibTeX field mapping tests ───────────────────────────────────────────────

#[test]
fn test_bib_record_fields_populated() {
    let work = WorkMeta {
        doi: "10.1234/test".to_string(),
        title: Some("My Article".to_string()),
        authors: vec!["Smith, Jane".to_string(), "Doe, John".to_string()],
        year: Some(2024),
        journal: Some("Nature".to_string()),
        volume: Some("42".to_string()),
        issue: Some("3".to_string()),
        pages: Some("1-10".to_string()),
        publisher: Some("Springer".to_string()),
        work_type: Some("journal-article".to_string()),
        ..WorkMeta::default()
    };
    let record = work_to_bib_record(&work);
    assert_eq!(record.entry_type, "article");
    assert_eq!(record.fields["title"], "My Article");
    assert_eq!(record.fields["author"], "Smith, Jane and Doe, John");
    assert_eq!(record.fields["year"], "2024");
    assert_eq!(record.fields["journal"], "Nature");
    assert_eq!(record.fields["volume"], "42");
    assert_eq!(record.fields["number"], "3");
    assert_eq!(record.fields["pages"], "1-10");
    assert_eq!(record.fields["publisher"], "Springer");
    assert_eq!(record.fields["doi"], "10.1234/test");
}

#[test]
fn test_work_type_mapping() {
    let types = vec![
        ("journal-article", "article"),
        ("book", "book"),
        ("monograph", "book"),
        ("book-chapter", "inbook"),
        ("proceedings-article", "inproceedings"),
        ("dissertation", "phdthesis"),
        ("report", "techreport"),
        ("unknown-type", "misc"),
    ];
    for (work_type, expected_bib_type) in types {
        let work = WorkMeta {
            doi: "10.1/x".to_string(),
            work_type: Some(work_type.to_string()),
            authors: vec!["Author, A".to_string()],
            year: Some(2020),
            ..WorkMeta::default()
        };
        let record = work_to_bib_record(&work);
        assert_eq!(
            record.entry_type, expected_bib_type,
            "work_type={work_type} should map to {expected_bib_type}"
        );
    }
}

// ─── append_to_bib_file tests ─────────────────────────────────────────────────

#[test]
fn test_append_deduplicates_by_doi() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path();

    let work = make_work("10.1234/alpha", "Alpha", 2020, "Alpha Paper");
    let record = work_to_bib_record(&work);

    // First append
    append_to_bib_file(path, &[record.clone()]).unwrap();
    let keys_after_first = parse_entry_keys(&std::fs::read_to_string(path).unwrap());
    assert_eq!(keys_after_first.len(), 1);

    // Second append of same DOI is a no-op
    append_to_bib_file(path, &[record]).unwrap();
    let keys_after_second = parse_entry_keys(&std::fs::read_to_string(path).unwrap());
    assert_eq!(keys_after_second.len(), 1, "duplicate DOI should not be appended");
}

#[test]
fn test_conflict_suffix_applied_within_batch() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path();

    // Two different DOIs that produce the same base citation key
    let work_a = make_work("10.1/x1", "Brown", 2023, "First Paper");
    let work_b = make_work("10.1/x2", "Brown", 2023, "Second Paper");

    let rec_a = work_to_bib_record(&work_a);
    let rec_b = work_to_bib_record(&work_b);
    // Both produce "Brown2023" as the base key

    append_to_bib_file(path, &[rec_a, rec_b]).unwrap();
    let content = std::fs::read_to_string(path).unwrap();
    let keys = parse_entry_keys(&content);

    assert_eq!(keys.len(), 2, "both records should be written");
    assert!(keys.contains(&"Brown2023".to_string()));
    assert!(keys.contains(&"Brown2023a".to_string()));
}

#[test]
fn test_conflict_suffix_across_calls() {
    let file = NamedTempFile::new().unwrap();
    let path = file.path();

    let work_a = make_work("10.1/y1", "Green", 2022, "First");
    let work_b = make_work("10.1/y2", "Green", 2022, "Second");

    let rec_a = work_to_bib_record(&work_a); // "Green2022"
    let rec_b = work_to_bib_record(&work_b); // "Green2022"

    // Append separately
    append_to_bib_file(path, &[rec_a]).unwrap();
    append_to_bib_file(path, &[rec_b]).unwrap();

    let content = std::fs::read_to_string(path).unwrap();
    let keys = parse_entry_keys(&content);

    assert_eq!(keys.len(), 2, "both records from separate calls should be written");
    assert!(keys.contains(&"Green2022".to_string()));
    assert!(keys.contains(&"Green2022a".to_string()));
}

#[test]
fn test_append_creates_file_if_not_exists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new.bib");

    let work = make_work("10.1/new", "New", 2024, "New Paper");
    let record = work_to_bib_record(&work);

    append_to_bib_file(&path, &[record]).unwrap();
    assert!(path.exists(), "file should be created");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("New2024"));
}

#[test]
fn test_records_to_bibtex_serialises_correctly() {
    let work = make_work("10.1/s", "Serialise", 2023, "Serialise Test");
    let record = work_to_bib_record(&work);
    let bib = records_to_bibtex(&[record]).unwrap();
    assert!(bib.contains("@article{Serialise2023"));
    assert!(bib.contains("doi = {10.1/s}"));
}
