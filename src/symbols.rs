use regex::Regex;

const ENGLISH_NOISE: &[&str] = &[
    "the", "this", "that", "with", "from", "what", "how", "why", "when",
    "where", "which", "who", "whom", "whose", "have", "has", "had", "not",
    "but", "for", "are", "was", "were", "been", "being", "some", "any",
    "each", "every", "both", "few", "more", "most", "other", "into",
    "over", "such", "than", "then", "through", "too", "very", "just",
        "about", "above", "after", "again", "all", "also", "and", "because",
        "before", "between", "down", "get", "got", "made", "much",
    "no", "off", "old", "only", "our", "out", "own", "per", "put",
    "may", "might", "must", "shall", "should", "will", "would", "can",
    "could", "do", "does", "did", "doing", "done", "say", "said",
    "see", "seen", "us", "use", "used", "using", "way", "ways",
    "back", "call", "called", "come", "could", "day", "days",
    "end", "ends", "even", "find", "first", "give", "good", "hand",
    "high", "know", "last", "leave", "left", "life", "like", "line",
        "long", "look", "made", "man", "men", "mean", "name",
    "name", "new", "next", "now", "old", "one", "part", "place",
    "right", "run", "same", "set", "show", "small", "state", "still",
    "take", "tell", "thing", "think", "three", "time", "top", "try",
    "turn", "two", "under", "upon", "well", "went", "work", "year",
    "years", "yet", "already", "always", "anything", "ask", "asked",
    "big", "bring", "brought", "change", "changed", "close", "common",
    "came", "far", "felt", "follow", "found", "general", "get", "gets",
    "getting", "going", "gone", "got", "group", "hand", "hands",
    "important", "keep", "kept", "kind", "knew", "large", "later",
    "least", "let", "lets", "little", "live", "lived", "lives",
        "looking", "looks", "making", "mean", "means", "meant",
    "move", "moved", "moves", "need", "needs", "never", "non", "nothing",
    "number", "numbers", "often", "once", "open", "opened", "order",
    "past", "point", "points", "present", "problem", "problems",
    "program", "programs", "real", "really", "reason", "reasons",
    "result", "results", "saw", "second", "seem", "seemed", "several",
    "short", "shown", "side", "sight", "since", "sort", "sorts",
    "special", "start", "started", "still", "stood", "stop", "sure",
    "take", "taken", "takes", "taking", "told", "took", "true", "turn",
    "turns", "used", "uses", "using", "value", "values", "various",
    "view", "want", "wanted", "wants", "ways", "within", "without",
    "words", "world", "worse", "worst", "write", "writes", "written",
    "wrote", "wrong", "went", "thing", "line", "helper", "helper",
    "util", "utils", "utility", "lib", "libs",
];

pub fn extract_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let q = query.trim();

    // 1. Call notation: `foo()`, `calculateTotal()`
    let call_re = Regex::new(r"(\w+)\s*\(\)").unwrap();
    for cap in call_re.captures_iter(q) {
        let name = cap[1].to_string();
        if seen.insert(name.clone()) {
            terms.push(name);
        }
    }

    // 2. Dotted path: `Container::make`, `Foo.bar()`
    let dot_re = Regex::new(r"[A-Za-z_]\w*(?:[.:]{2})[A-Za-z_]\w*").unwrap();
    for m in dot_re.find_iter(q) {
        for part in m.as_str().split(|c| c == ':' || c == '.') {
            if !part.is_empty() && !ENGLISH_NOISE.contains(&part) && seen.insert(part.to_string()) {
                terms.push(part.to_string());
            }
        }
    }

    // 3. PascalCase / camelCase / SCREAMING_CASE
    let word_re = Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").unwrap();
    for m in word_re.find_iter(q) {
        let word = m.as_str();
        if word.len() < 2 { continue; }
        if ENGLISH_NOISE.contains(&word) { continue; }
        if seen.insert(word.to_string()) {
            terms.push(word.to_string());
        }
    }

    // 4. Keyword-before pattern: "the X method", "the X function"
    let before_re = Regex::new(r"(?:the|the\s+|\b)\b(\w+)\s+(?:method|function|class|trait|struct|enum|fn|def)\b").unwrap();
    for cap in before_re.captures_iter(q) {
        let name = cap[1].to_string();
        if !ENGLISH_NOISE.contains(&name.as_str()) && seen.insert(name.clone()) {
            terms.push(name);
        }
    }

    // 5. Keyword-after pattern: "method X", "function foo"
    let after_re = Regex::new(r"(?:method|function|class|trait|struct|enum|fn|def)\s+(\w+)").unwrap();
    for cap in after_re.captures_iter(q) {
        let name = cap[1].to_string();
        if !ENGLISH_NOISE.contains(&name.as_str()) && seen.insert(name.clone()) {
            terms.push(name);
        }
    }

    terms.sort();
    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_notation() {
        let t = extract_terms("buildMiniGraph()");
        assert!(t.contains(&"buildMiniGraph".to_string()));
    }

    #[test]
    fn test_dotted_path() {
        let t = extract_terms("Container::make");
        assert!(t.contains(&"Container".to_string()));
        assert!(t.contains(&"make".to_string()));
    }

    #[test]
    fn test_camel_case() {
        let t = extract_terms("what does lazyBuildContext do");
        assert!(t.contains(&"lazyBuildContext".to_string()));
    }

    #[test]
    fn test_pascal_case() {
        let t = extract_terms("the CodeGraph class");
        assert!(t.contains(&"CodeGraph".to_string()));
    }

    #[test]
    fn test_keyword_before() {
        let t = extract_terms("the make method");
        assert!(t.contains(&"make".to_string()));
    }

    #[test]
    fn test_keyword_after() {
        let t = extract_terms("find the function walkDir");
        assert!(t.contains(&"walkDir".to_string()));
    }

    #[test]
    fn test_english_noise_filtered() {
        let t = extract_terms("the this method");
        assert!(!t.contains(&"this".to_string()));
        assert!(!t.contains(&"the".to_string()));
    }

    #[test]
    fn test_empty() {
        let t = extract_terms("");
        assert!(t.is_empty());
    }

    #[test]
    fn test_noise_only() {
        let t = extract_terms("how does the helper work");
        assert!(t.is_empty() || !t.iter().any(|w| ENGLISH_NOISE.contains(&w.as_str())));
    }

    #[test]
    fn test_scream_case() {
        let t = extract_terms("MAX_GREP_RESULTS");
        assert!(t.contains(&"MAX_GREP_RESULTS".to_string()));
    }

    #[test]
    fn test_no_duplicates() {
        let t = extract_terms("walkDir walkDir walkDir");
        assert_eq!(t.iter().filter(|&x| x == "walkDir").count(), 1);
    }
}
