use crate::models::WorkMeta;

/// Citation key generation style.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyStyle {
    /// Author last-name(s) + year, e.g. `Smith2024`.
    AuthorYear,
    /// Significant title words + year, e.g. `MachineLearning2024`.
    ShortTitle,
}

/// Generate a citation key according to the given style.
pub fn generate_citation_key_by_style(work: &WorkMeta, style: &KeyStyle) -> String {
    match style {
        KeyStyle::AuthorYear => generate_citation_key(&work.authors, work.year),
        KeyStyle::ShortTitle => generate_short_title_key(work),
    }
}

/// Generate an author-year citation key, e.g. `Smith2024` or `SmithJones2024`.
///
/// Collision suffixes (`a`, `b`, …) must be resolved by the caller after checking
/// the existing keys in a bibliography.
pub fn generate_citation_key(authors: &[String], year: Option<i32>) -> String {
    let author_part = authors
        .iter()
        .take(2)
        .filter_map(|a| {
            // Authors are stored as "Family, Given" or just "Family"
            a.split(',')
                .next()
                .map(|family| family.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .map(capitalise_first)
        .collect::<Vec<_>>()
        .join("");

    let year_part = year
        .map(|y| y.to_string())
        .unwrap_or_default();

    if author_part.is_empty() {
        format!("Unknown{year_part}")
    } else {
        format!("{author_part}{year_part}")
    }
}

/// Append a conflict suffix (`a`, `b`, …) to `base_key` until it is unique
/// among `existing_keys`.
pub fn resolve_key_conflict(base_key: &str, existing_keys: &[String]) -> String {
    if !existing_keys.contains(&base_key.to_string()) {
        return base_key.to_string();
    }
    (b'a'..=b'z')
        .map(|c| format!("{}{}", base_key, c as char))
        .find(|candidate| !existing_keys.contains(candidate))
        .unwrap_or_else(|| {
            // Beyond 'z': try two-letter suffixes aa, ab, …
            for c1 in b'a'..=b'z' {
                for c2 in b'a'..=b'z' {
                    let candidate = format!("{}{}{}", base_key, c1 as char, c2 as char);
                    if !existing_keys.contains(&candidate) {
                        return candidate;
                    }
                }
            }
            format!("{base_key}_conflict")
        })
}

/// Normalise a raw DOI string: strip URL prefixes if present.
///
/// E.g. `https://doi.org/10.1234/test` → `10.1234/test`
pub fn normalise_doi(doi: &str) -> String {
    doi.trim()
        .trim_start_matches("https://doi.org/")
        .trim_start_matches("http://doi.org/")
        .trim_start_matches("https://dx.doi.org/")
        .trim_start_matches("http://dx.doi.org/")
        .trim_start_matches("doi:")
        .to_string()
}

/// Capitalise the first character of a string, leaving the rest unchanged.
pub fn capitalise_first(s: String) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Generate a short-title citation key, e.g. `MachineLearning2024`.
///
/// Strips common stop-words, takes the first four significant words,
/// capitalises each, and appends the year.
fn generate_short_title_key(work: &WorkMeta) -> String {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "of", "in", "on", "at", "to", "for", "and",
        "or", "by", "with", "is", "are", "was", "were", "from", "as",
        "into", "that", "this", "its", "be", "has", "have", "had",
    ];

    let title_part: String = work
        .title
        .as_deref()
        .unwrap_or("")
        .split_whitespace()
        .filter(|w| {
            // Strip leading/trailing punctuation for the stop-word check
            let lower: String = w
                .chars()
                .filter(|c| c.is_alphabetic())
                .collect::<String>()
                .to_lowercase();
            !lower.is_empty() && !STOP_WORDS.contains(&lower.as_str())
        })
        .take(4)
        .map(|w| {
            // Keep only alphanumeric characters, then capitalise
            let clean: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
            capitalise_first(clean)
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("");

    let year_part = work.year.map(|y| y.to_string()).unwrap_or_default();

    if title_part.is_empty() {
        format!("Unknown{year_part}")
    } else {
        format!("{title_part}{year_part}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalise_doi_strips_url() {
        assert_eq!(
            normalise_doi("https://doi.org/10.1234/test"),
            "10.1234/test"
        );
        assert_eq!(normalise_doi("10.1234/test"), "10.1234/test");
        assert_eq!(normalise_doi("doi:10.1234/test"), "10.1234/test");
    }

    #[test]
    fn test_generate_citation_key_single_author() {
        let authors = vec!["Smith, John".to_string()];
        assert_eq!(generate_citation_key(&authors, Some(2024)), "Smith2024");
    }

    #[test]
    fn test_generate_citation_key_two_authors() {
        let authors = vec!["Smith, John".to_string(), "Jones, Alice".to_string()];
        assert_eq!(generate_citation_key(&authors, Some(2024)), "SmithJones2024");
    }

    #[test]
    fn test_resolve_key_conflict() {
        let existing = vec!["Smith2024".to_string(), "Smith2024a".to_string()];
        assert_eq!(resolve_key_conflict("Smith2024", &existing), "Smith2024b");
    }

    #[test]
    fn test_resolve_key_conflict_beyond_z() {
        // Build a list with all single-letter suffixes occupied
        let mut existing = vec!["Smith2024".to_string()];
        for c in b'a'..=b'z' {
            existing.push(format!("Smith2024{}", c as char));
        }
        // Should fall back to two-letter suffix "aa"
        assert_eq!(resolve_key_conflict("Smith2024", &existing), "Smith2024aa");
    }

    #[test]
    fn test_short_title_key_style() {
        let work = WorkMeta {
            doi: "10.1234/ml".to_string(),
            title: Some("Machine Learning in Practice".to_string()),
            authors: vec!["Smith, John".to_string()],
            year: Some(2024),
            ..WorkMeta::default()
        };
        let key = generate_citation_key_by_style(&work, &KeyStyle::ShortTitle);
        // "Machine", "Learning", "Practice" (stop words: "in")
        assert_eq!(key, "MachineLearningPractice2024");
    }

    #[test]
    fn test_short_title_key_strips_stop_words() {
        let work = WorkMeta {
            doi: "10.1234/a".to_string(),
            title: Some("The Role of AI in the Future".to_string()),
            authors: vec![],
            year: Some(2020),
            ..WorkMeta::default()
        };
        let key = generate_citation_key_by_style(&work, &KeyStyle::ShortTitle);
        // Stop words removed: "The", "of", "in", "the"
        // Remaining: "Role", "AI", "Future" (only 3 significant words)
        assert_eq!(key, "RoleAIFuture2020");
    }

    #[test]
    fn test_author_year_key_style() {
        let work = WorkMeta {
            doi: "10.1234/t".to_string(),
            title: Some("Some Title".to_string()),
            authors: vec!["Smith, John".to_string()],
            year: Some(2024),
            ..WorkMeta::default()
        };
        let key = generate_citation_key_by_style(&work, &KeyStyle::AuthorYear);
        assert_eq!(key, "Smith2024");
    }
}
