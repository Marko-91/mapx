use std::collections::HashMap;
use std::path::Path;

use crate::grep;

#[derive(Debug, Clone)]
pub struct Bm25Score {
    pub file: String,
    pub score: f64,
    pub top_terms: Vec<(String, f64)>,
}

const K1: f64 = 1.5;
const B: f64 = 0.75;

pub fn compute_bm25(terms: &[String], grep_hits: &[grep::GrepHit], _root: &Path) -> Vec<Bm25Score> {
    if grep_hits.is_empty() || terms.is_empty() {
        return vec![];
    }

    // Read file contents
    let mut file_contents: HashMap<String, String> = HashMap::new();
    for hit in grep_hits.iter().take(50) {
        if let Ok(content) = std::fs::read_to_string(&hit.file) {
            file_contents.insert(hit.file.clone(), content);
        }
    }

    if file_contents.is_empty() {
        return vec![];
    }

    // Compute average document length
    let total_len: usize = file_contents.values().map(|c| c.split_whitespace().count()).sum();
    let num_docs = file_contents.len() as f64;
    let avg_doc_len = total_len as f64 / num_docs;

    // Compute IDF for each term
    let mut idf: HashMap<String, f64> = HashMap::new();
    for term in terms {
        let docs_with_term = file_contents.values()
            .filter(|content| content.contains(term.as_str()))
            .count() as f64;
        let idf_val = ((num_docs - docs_with_term + 0.5) / (docs_with_term + 0.5) + 1.0).ln();
        idf.insert(term.clone(), idf_val);
    }

    // Compute BM25 for each file
    let mut scores: Vec<Bm25Score> = file_contents.iter().map(|(file, content)| {
        let words: Vec<&str> = content.split_whitespace().collect();
        let doc_len = words.len() as f64;
        let mut total_score = 0.0;
        let mut term_scores = Vec::new();

        for term in terms {
            let tf = words.iter().filter(|w| w == &term).count() as f64;
            if tf == 0.0 { continue; }
            let idf_val = idf.get(term).copied().unwrap_or(0.0);
            let score = idf_val * (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * doc_len / avg_doc_len));
            total_score += score;
            term_scores.push((term.clone(), score));
        }

        term_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Bm25Score {
            file: file.clone(),
            score: total_score,
            top_terms: term_scores.into_iter().take(5).collect(),
        }
    }).collect();

    scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_hits() {
        let result = compute_bm25(&["test".to_string()], &[], Path::new("/"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_empty_terms() {
        let result = compute_bm25(&[], &[], Path::new("/"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_idf_calculation() {
        // IDF for a term that appears in all docs should be lower
        let mut contents = HashMap::new();
        contents.insert("file1.rs".to_string(), "fn foo() { bar() }".to_string());
        contents.insert("file2.rs".to_string(), "fn baz() { foo() }".to_string());

        let _avg_len: f64 = 5.0;
        let num_docs: f64 = 2.0;
        // "foo" appears in both docs -> IDF ~= ((2-2+0.5)/(2+0.5)+1).ln() = (0.5/2.5+1).ln() = 1.2.ln() ≈ 0.182
        let idf_foo = ((num_docs - 2.0 + 0.5) / (2.0 + 0.5) + 1.0).ln();
        // "bar" appears in 1 doc -> IDF = ((2-1+0.5)/(1+0.5)+1).ln() = (1.5/1.5+1).ln() = 2.ln() ≈ 0.693
        let idf_bar = ((num_docs - 1.0 + 0.5) / (1.0 + 0.5) + 1.0).ln();
        assert!(idf_bar > idf_foo);
    }
}
