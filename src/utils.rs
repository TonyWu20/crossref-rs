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
        format!("Unknown{}", year_part)
    } else {
        format!("{}{}", author_part, year_part)
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
        .unwrap_or_else(|| format!("{}_conflict", base_key))
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
fn capitalise_first(s: String) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
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
}
