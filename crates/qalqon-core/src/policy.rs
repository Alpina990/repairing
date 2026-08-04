use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentDecision {
    Allow,
    Block { matched_term: String },
}

pub struct ContentPolicy;

impl ContentPolicy {
    pub fn evaluate(text: &str, blocked_terms: &[String]) -> ContentDecision {
        let normalized = normalize(text);
        blocked_terms
            .iter()
            .find(|term| {
                let term = normalize(term);
                !term.is_empty() && normalized.contains(&term)
            })
            .map_or(ContentDecision::Allow, |term| ContentDecision::Block {
                matched_term: term.clone(),
            })
    }
}

fn normalize(value: &str) -> String {
    value.nfkc().collect::<String>().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_case_insensitively() {
        let terms = vec!["ReKlAmA".to_string()];
        assert_eq!(
            ContentPolicy::evaluate("Bu REKLAMA xabari", &terms),
            ContentDecision::Block {
                matched_term: "ReKlAmA".into()
            }
        );
    }

    #[test]
    fn empty_terms_never_match() {
        assert_eq!(
            ContentPolicy::evaluate("hello", &[String::new()]),
            ContentDecision::Allow
        );
    }
}
