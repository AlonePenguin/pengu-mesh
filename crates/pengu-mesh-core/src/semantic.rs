use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

const ROLE_BOOST_PER_MATCH: f64 = 0.12;
const ROLE_BOOST_CAP: f64 = 0.25;
const SYNONYM_BOOST_WEIGHT: f64 = 0.30;
const PREFIX_MATCH_WEIGHT: f64 = 0.20;

const STOPWORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "could", "should", "may", "might", "shall", "can", "to",
    "of", "for", "with", "at", "by", "from", "as", "into", "through", "about", "above", "after",
    "before", "between", "under", "and", "but", "nor", "so", "yet", "both", "either", "neither",
    "this", "that", "these", "those", "it", "its", "i", "me", "my", "we", "our", "you", "your",
    "he", "she", "his", "her", "they", "their",
];

const SEMANTIC_STOPWORDS: &[&str] = &["in", "up", "out", "on", "off", "not", "no", "or", "ok"];

const ROLE_KEYWORDS: &[&str] = &[
    "button", "input", "link", "submit", "form", "textbox", "checkbox", "radio", "select",
    "option", "tab", "menu", "search",
];

const SYNONYM_GROUPS: &[(&str, &[&str])] = &[
    (
        "login",
        &[
            "signin",
            "log in",
            "sign in",
            "authenticate",
            "logon",
            "log on",
        ],
    ),
    ("logout", &["signout", "log out", "sign out", "logoff"]),
    (
        "register",
        &["signup", "sign up", "create account", "join", "enroll"],
    ),
    ("password", &["passcode", "passphrase", "pwd"]),
    (
        "username",
        &["userid", "user name", "user id", "login name"],
    ),
    ("email", &["e-mail", "mail", "email address"]),
    ("forgot", &["reset", "recover", "lost"]),
    ("search", &["find", "lookup", "look up", "query", "filter"]),
    ("menu", &["navigation", "nav", "sidebar", "hamburger"]),
    ("home", &["homepage", "main page", "start", "landing"]),
    ("back", &["return", "go back", "previous"]),
    ("next", &["continue", "proceed", "forward", "advance"]),
    ("previous", &["prev", "back", "prior"]),
    ("close", &["dismiss", "exit", "x", "cancel"]),
    ("open", &["expand", "show", "reveal"]),
    (
        "settings",
        &["preferences", "options", "configuration", "config"],
    ),
    (
        "submit",
        &["send", "confirm", "apply", "save", "done", "go"],
    ),
    ("cancel", &["abort", "discard", "nevermind"]),
    ("edit", &["modify", "change", "update"]),
    ("delete", &["remove", "erase", "trash", "discard"]),
    ("add", &["create", "new", "insert", "plus"]),
    ("upload", &["attach", "choose file", "browse"]),
    ("download", &["export", "save as", "get"]),
    ("button", &["btn", "cta"]),
    ("input", &["field", "textbox", "text box", "text field"]),
    (
        "dropdown",
        &["select", "combobox", "combo box", "picker", "listbox"],
    ),
    ("checkbox", &["check box", "tick", "toggle"]),
    ("link", &["anchor", "hyperlink", "href"]),
    ("tab", &["panel", "pane"]),
    (
        "modal",
        &["dialog", "dialogue", "popup", "pop up", "overlay"],
    ),
    ("notification", &["alert", "toast", "banner", "message"]),
    ("tooltip", &["hint", "info", "help text"]),
    (
        "avatar",
        &["profile picture", "profile pic", "user image", "photo"],
    ),
    ("cart", &["basket", "bag", "shopping cart"]),
    (
        "checkout",
        &["pay", "purchase", "buy", "place order", "order"],
    ),
    ("price", &["cost", "amount", "total"]),
    ("quantity", &["qty", "count", "amount"]),
    ("image", &["img", "picture", "photo", "icon"]),
    ("video", &["clip", "media", "player"]),
    ("title", &["heading", "header", "headline"]),
    ("description", &["desc", "summary", "subtitle", "caption"]),
    ("list", &["items", "collection", "grid"]),
    ("click", &["press", "tap", "hit", "select"]),
    ("scroll", &["swipe", "slide"]),
    ("drag", &["move", "reorder"]),
    ("copy", &["duplicate", "clone"]),
    ("paste", &["insert"]),
    ("undo", &["revert", "rollback"]),
    ("redo", &["repeat"]),
    ("refresh", &["reload", "update"]),
    ("share", &["send", "forward"]),
    (
        "like",
        &["favorite", "favourite", "heart", "star", "upvote"],
    ),
    (
        "accept",
        &["agree", "allow", "ok", "okay", "yes", "confirm"],
    ),
    ("reject", &["deny", "decline", "refuse", "no"]),
];

static STOPWORD_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| STOPWORDS.iter().copied().collect());
static SEMANTIC_STOPWORD_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| SEMANTIC_STOPWORDS.iter().copied().collect());
static ROLE_KEYWORD_SET: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| ROLE_KEYWORDS.iter().copied().collect());
static SYNONYM_INDEX: LazyLock<HashMap<&'static str, HashSet<&'static str>>> =
    LazyLock::new(build_synonym_index);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Phrase {
    text: String,
    start_idx: usize,
    end_idx: usize,
}

fn build_synonym_index() -> HashMap<&'static str, HashSet<&'static str>> {
    let mut index = HashMap::new();

    for (canonical, synonyms) in SYNONYM_GROUPS {
        index.entry(*canonical).or_insert_with(HashSet::new);
        for synonym in *synonyms {
            index
                .entry(*canonical)
                .or_insert_with(HashSet::new)
                .insert(*synonym);
        }

        for synonym in *synonyms {
            let synonym_entry = index.entry(*synonym).or_insert_with(HashSet::new);
            synonym_entry.insert(*canonical);
            for other in *synonyms {
                if synonym != other {
                    synonym_entry.insert(*other);
                }
            }
        }
    }

    index
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(String::from)
        .collect()
}

fn build_phrases(tokens: &[String], max_n: usize) -> Vec<Phrase> {
    let mut phrases = Vec::new();
    for n in 2..=max_n.min(tokens.len()) {
        for start_idx in 0..=tokens.len() - n {
            phrases.push(Phrase {
                text: tokens[start_idx..start_idx + n].join(" "),
                start_idx,
                end_idx: start_idx + n - 1,
            });
        }
    }
    phrases
}

fn remove_stopwords_context_aware(tokens: &[String], other_tokens: &[String]) -> Vec<String> {
    let other_set: HashSet<&str> = other_tokens.iter().map(String::as_str).collect();
    let mut phrase_tokens = HashSet::new();

    for phrase in build_phrases(tokens, 3) {
        if SYNONYM_INDEX.contains_key(phrase.text.as_str()) {
            for idx in phrase.start_idx..=phrase.end_idx {
                phrase_tokens.insert(idx);
            }
        }
    }

    let mut filtered = Vec::with_capacity(tokens.len());
    for (idx, token) in tokens.iter().enumerate() {
        let token_str = token.as_str();
        let keep = (!STOPWORD_SET.contains(token_str)
            && !SEMANTIC_STOPWORD_SET.contains(token_str))
            || phrase_tokens.contains(&idx)
            || (SEMANTIC_STOPWORD_SET.contains(token_str) && other_set.contains(token_str))
            || (SEMANTIC_STOPWORD_SET.contains(token_str) && SYNONYM_INDEX.contains_key(token_str));

        if keep {
            filtered.push(token.clone());
        }
    }

    if filtered.is_empty() {
        return tokens.to_vec();
    }

    filtered
}

fn token_frequencies(tokens: &[String]) -> HashMap<&str, usize> {
    let mut frequencies = HashMap::with_capacity(tokens.len());
    for token in tokens {
        *frequencies.entry(token.as_str()).or_insert(0) += 1;
    }
    frequencies
}

fn weighted_jaccard_similarity(a: &[String], b: &[String]) -> f64 {
    let freq_a = token_frequencies(a);
    let freq_b = token_frequencies(b);

    let mut intersection = 0.0;
    for (token, count_a) in &freq_a {
        if let Some(count_b) = freq_b.get(token) {
            intersection += (*count_a).min(*count_b) as f64;
        }
    }

    let mut all_tokens: HashSet<&str> = freq_a.keys().copied().collect();
    all_tokens.extend(freq_b.keys().copied());

    let mut union = 0.0;
    for token in all_tokens {
        let count_a = freq_a.get(token).copied().unwrap_or_default();
        let count_b = freq_b.get(token).copied().unwrap_or_default();
        union += count_a.max(count_b) as f64;
    }

    if union == 0.0 {
        return 0.0;
    }

    intersection / union
}

fn synonym_score(query_tokens: &[String], candidate_tokens: &[String]) -> f64 {
    if query_tokens.is_empty() || candidate_tokens.is_empty() {
        return 0.0;
    }

    let candidate_set: HashSet<&str> = candidate_tokens.iter().map(String::as_str).collect();
    let mut matched = 0usize;
    let mut consumed_indices = HashSet::new();

    for phrase in build_phrases(query_tokens, 3) {
        if let Some(synonyms) = SYNONYM_INDEX.get(phrase.text.as_str()) {
            for synonym in synonyms {
                let synonym_tokens: Vec<&str> = synonym.split_whitespace().collect();
                if synonym_tokens
                    .iter()
                    .all(|token| candidate_set.contains(token))
                {
                    matched += 1;
                    for idx in phrase.start_idx..=phrase.end_idx {
                        consumed_indices.insert(idx);
                    }
                    break;
                }
            }
        }
    }

    for (idx, token) in query_tokens.iter().enumerate() {
        if consumed_indices.contains(&idx) || candidate_set.contains(token.as_str()) {
            continue;
        }

        if let Some(synonyms) = SYNONYM_INDEX.get(token.as_str()) {
            for synonym in synonyms {
                let synonym_tokens: Vec<&str> = synonym.split_whitespace().collect();
                if synonym_tokens
                    .iter()
                    .all(|synonym_token| candidate_set.contains(synonym_token))
                {
                    matched += 1;
                    break;
                }
            }
        }
    }

    matched as f64 / query_tokens.len() as f64
}

fn token_prefix_score(query_tokens: &[String], candidate_tokens: &[String]) -> f64 {
    if query_tokens.is_empty() {
        return 0.0;
    }

    let mut total = 0.0;

    for query_token in query_tokens {
        if query_token.len() < 2 {
            continue;
        }

        let mut best_match = 0.0_f64;
        for candidate_token in candidate_tokens {
            if query_token == candidate_token {
                continue;
            }

            if candidate_token.len() > query_token.len() && candidate_token.starts_with(query_token)
            {
                let ratio = query_token.len() as f64 / candidate_token.len() as f64;
                best_match = best_match.max(ratio);
            }

            if query_token.len() > candidate_token.len() && query_token.starts_with(candidate_token)
            {
                let ratio = candidate_token.len() as f64 / query_token.len() as f64;
                best_match = best_match.max(ratio * 0.7);
            }
        }

        total += best_match;
    }

    total / query_tokens.len() as f64
}

fn role_overlap_boost(query_tokens: &[String], candidate_tokens: &[String]) -> f64 {
    let query_set: HashSet<&str> = query_tokens.iter().map(String::as_str).collect();
    let candidate_set: HashSet<&str> = candidate_tokens.iter().map(String::as_str).collect();
    let mut boost = 0.0;

    for token in &query_set {
        if ROLE_KEYWORD_SET.contains(token) && candidate_set.contains(token) {
            boost += ROLE_BOOST_PER_MATCH;
        }
    }

    for token in &query_set {
        if ROLE_KEYWORD_SET.contains(token) {
            continue;
        }

        if let Some(synonyms) = SYNONYM_INDEX.get(token) {
            if synonyms.iter().any(|synonym| {
                ROLE_KEYWORD_SET.contains(synonym) && candidate_set.contains(synonym)
            }) {
                boost += ROLE_BOOST_PER_MATCH * 0.8;
            }
        }
    }

    boost.min(ROLE_BOOST_CAP)
}

fn lexical_score(query: &str, candidate: &str) -> f64 {
    let raw_query_tokens = tokenize(query);
    let raw_candidate_tokens = tokenize(candidate);

    let query_tokens = remove_stopwords_context_aware(&raw_query_tokens, &raw_candidate_tokens);
    let candidate_tokens = remove_stopwords_context_aware(&raw_candidate_tokens, &raw_query_tokens);

    if query_tokens.is_empty() || candidate_tokens.is_empty() {
        return 0.0;
    }

    let base_score = weighted_jaccard_similarity(&query_tokens, &candidate_tokens);
    let synonym_boost = synonym_score(&query_tokens, &candidate_tokens)
        .max(synonym_score(&candidate_tokens, &query_tokens))
        * SYNONYM_BOOST_WEIGHT;
    let prefix_boost = token_prefix_score(&query_tokens, &candidate_tokens)
        .max(token_prefix_score(&candidate_tokens, &query_tokens))
        * PREFIX_MATCH_WEIGHT;
    let role_boost = role_overlap_boost(&query_tokens, &candidate_tokens)
        .max(role_overlap_boost(&candidate_tokens, &query_tokens));

    (base_score + synonym_boost + prefix_boost + role_boost).min(1.0)
}

/// Lexical matcher for semantic element lookup using zero-dependency token
/// overlap, synonym expansion, and role-aware boosts.
#[derive(Debug, Clone, Copy)]
pub struct LexicalMatcher {
    threshold: f64,
}

impl Default for LexicalMatcher {
    fn default() -> Self {
        Self { threshold: 0.3 }
    }
}

impl LexicalMatcher {
    pub fn new(threshold: f64) -> Self {
        Self { threshold }
    }

    pub fn score(&self, query: &str, candidate: &str) -> f64 {
        lexical_score(query, candidate)
    }

    pub fn matches(&self, query: &str, candidate: &str) -> bool {
        self.score(query, candidate) >= self.threshold
    }

    pub fn best_match(&self, query: &str, candidates: &[String]) -> Option<(usize, f64)> {
        let mut best = None;

        for (idx, candidate) in candidates.iter().enumerate() {
            let score = self.score(query, candidate);
            if score >= self.threshold && score > best.map_or(0.0, |(_, best_score)| best_score) {
                best = Some((idx, score));
            }
        }

        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_lowercases_and_splits() {
        let tokens = tokenize("Sign-In Button");
        assert_eq!(tokens, vec!["sign", "in", "button"]);
    }

    #[test]
    fn context_stopwords_preserve_semantic_phrases() {
        let tokens = tokenize("sign in");
        let other_tokens = tokenize("login button");
        let filtered = remove_stopwords_context_aware(&tokens, &other_tokens);
        assert_eq!(filtered, vec!["sign", "in"]);
    }

    #[test]
    fn context_stopwords_drop_noise_but_keep_signal() {
        let tokens = tokenize("click on the submit button");
        let other_tokens = tokenize("submit button");
        let filtered = remove_stopwords_context_aware(&tokens, &other_tokens);
        assert_eq!(filtered, vec!["click", "submit", "button"]);
    }

    #[test]
    fn lexical_score_respects_phrase_synonyms() {
        let score = lexical_score("sign in button", "login button");
        assert!(score > 0.5, "score was {score}");
    }

    #[test]
    fn lexical_score_respects_prefix_matches() {
        let score = lexical_score("nav menu", "navigation menu");
        assert!(score > 0.5, "score was {score}");
    }

    #[test]
    fn lexical_score_caps_at_one() {
        let score = lexical_score("submit button", "submit button");
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn weighted_jaccard_counts_duplicate_tokens() {
        let a = vec![
            "submit".to_string(),
            "submit".to_string(),
            "button".to_string(),
        ];
        let b = vec!["submit".to_string(), "button".to_string()];
        let score = weighted_jaccard_similarity(&a, &b);
        assert!((score - (2.0 / 3.0)).abs() < 1e-9);
    }

    #[test]
    fn synonym_score_avoids_double_counting_phrase_components() {
        let score = synonym_score(&tokenize("sign in"), &tokenize("login"));
        assert!((score - 0.5).abs() < 1e-9);
    }

    #[test]
    fn default_threshold_is_stable() {
        let matcher = LexicalMatcher::default();
        assert!((matcher.threshold - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn matches_identical_text() {
        let matcher = LexicalMatcher::default();
        assert!(matcher.matches("submit button", "submit button"));
    }

    #[test]
    fn matches_via_synonym_and_role_overlap() {
        let matcher = LexicalMatcher::default();
        assert!(matcher.matches("button", "btn"));
    }

    #[test]
    fn rejects_unrelated_text() {
        let matcher = LexicalMatcher::default();
        assert!(!matcher.matches("submit button", "shopping cart"));
    }

    #[test]
    fn score_is_symmetric() {
        let matcher = LexicalMatcher::default();
        let left = matcher.score("input field", "textbox element");
        let right = matcher.score("textbox element", "input field");
        assert!((left - right).abs() < 1e-9);
    }

    #[test]
    fn best_match_prefers_highest_score_above_threshold() {
        let matcher = LexicalMatcher::default();
        let candidates = vec![
            "navigation menu".to_string(),
            "submit btn".to_string(),
            "footer text".to_string(),
        ];
        let result = matcher.best_match("submit button", &candidates);
        assert_eq!(result.map(|(idx, _)| idx), Some(1));
    }

    #[test]
    fn best_match_returns_none_below_threshold() {
        let matcher = LexicalMatcher::new(0.99);
        let candidates = vec!["totally unrelated stuff".to_string()];
        assert!(matcher.best_match("submit button", &candidates).is_none());
    }

    #[test]
    fn empty_stopword_only_input_falls_back_to_original_tokens() {
        let tokens = tokenize("the a an");
        let filtered = remove_stopwords_context_aware(&tokens, &[]);
        assert_eq!(filtered, tokens);
    }
}
