pub mod symbols;
pub mod grep;
pub mod bm25;
pub mod graph;
pub mod output;
pub mod callgraph;

#[cfg(feature = "ranker")]
pub mod ranker;

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub rel_fname: String,
    pub fname: String,
    pub line: usize,
    pub name: String,
    pub kind: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PipelineMode {
    Grep,
    Bm25,
    Full,
}

impl PipelineMode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "grep" => PipelineMode::Grep,
            "bm25" => PipelineMode::Bm25,
            _ => PipelineMode::Full,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineResult {
    pub tags: Vec<Tag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_graph: Option<Vec<callgraph::CallEdge>>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub root: PathBuf,
    pub query: String,
    pub mode: PipelineMode,
    pub max_results: usize,
    pub rank_model: Option<String>,
    pub ollama_base: String,
    pub format: String,
    pub lang_dir: Option<PathBuf>,
    pub call_graph: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            root: PathBuf::from("."),
            query: String::new(),
            mode: PipelineMode::Full,
            max_results: 20,
            rank_model: None,
            ollama_base: "http://localhost:11434".to_string(),
            format: "json".to_string(),
            lang_dir: None,
            call_graph: false,
        }
    }
}

pub fn run_pipeline(config: &Config) -> Result<PipelineResult, String> {
    let query = config.query.trim();
    if query.is_empty() {
        return Ok(PipelineResult { tags: vec![], call_graph: None });
    }

    // 1. Symbols
    let terms = symbols::extract_terms(query);
    if terms.is_empty() {
        return Ok(PipelineResult { tags: vec![], call_graph: None });
    }

    // 2. Grep
    let ext_patterns = grep::load_language_patterns(&config.root, config.lang_dir.as_deref())?;
    let grep_hits = grep::smart_grep(&config.root, &terms, &ext_patterns);
    if grep_hits.is_empty() {
        return Ok(PipelineResult { tags: vec![], call_graph: None });
    }

    if config.mode == PipelineMode::Grep {
        let tags = build_tags(&grep_hits, None, &config.root);
        let cg = if config.call_graph { Some(callgraph::build_call_graph(&grep_hits)) } else { None };
        return Ok(PipelineResult { tags, call_graph: cg });
    }

    // 3. BM25
    let bm25_scores = bm25::compute_bm25(&terms, &grep_hits, &config.root);

    // 4. Merge grep + BM25 candidates
    let merged = merge_candidates(&grep_hits, &bm25_scores);

    if config.mode == PipelineMode::Bm25 || merged.is_empty() {
        let tags = build_tags_from_merged(&merged, &config.root);
        let cg = if config.call_graph { Some(callgraph::build_call_graph(&grep_hits)) } else { None };
        return Ok(PipelineResult { tags, call_graph: cg });
    }

    // 5. Graph BFS
    let tags = graph::build_graph_tags(&merged, &terms, &config.root);

    // 6. Call graph (optional)
    let cg = if config.call_graph { Some(callgraph::build_call_graph(&grep_hits)) } else { None };

    // 7. Optional LLM rank
    #[cfg(feature = "ranker")]
    if let Some(ref model) = config.rank_model {
        if !tags.is_empty() {
            let ranked = ranker::llm_rank(&tags, model, &config.ollama_base);
            let ranked_filtered: Vec<Tag> = ranked.into_iter().filter(|t| t.score >= 1.0).collect();
            return Ok(PipelineResult { tags: ranked_filtered, call_graph: cg });
        }
    }

    let tags: Vec<Tag> = tags.into_iter().filter(|t| t.score >= 1.0).collect();
    Ok(PipelineResult { tags, call_graph: cg })
}

#[derive(Debug, Clone, Serialize)]
pub struct MergedCandidate {
    pub file: String,
    pub grep_score: f64,
    pub best_priority: u32,
    pub roles: Vec<String>,
    pub matches: Vec<grep::MatchInfo>,
    pub bm25_score: f64,
}

impl MergedCandidate {
    pub fn combined_score(&self) -> f64 {
        self.grep_score * 2.0 + self.bm25_score * 1000.0
    }
}

fn merge_candidates(grep_hits: &[grep::GrepHit], bm25_scores: &[bm25::Bm25Score]) -> Vec<MergedCandidate> {
    let grep_top: std::collections::HashSet<&str> = grep_hits.iter().filter(|g| g.score > 0.0).map(|g| g.file.as_str()).collect();
    let bm25_top: std::collections::HashSet<&str> = bm25_scores.iter().filter(|b| b.score > 0.0).map(|b| b.file.as_str()).collect();
    let union: std::collections::HashSet<&str> = grep_top.union(&bm25_top).copied().collect();

    let mut merged: Vec<MergedCandidate> = union.iter().map(|&fp| {
        let g = grep_hits.iter().find(|g| g.file == fp);
        let b = bm25_scores.iter().find(|b| b.file == fp);
        MergedCandidate {
            file: fp.to_string(),
            grep_score: g.map(|g| g.score).unwrap_or(0.0),
            best_priority: g.map(|g| g.best_priority).unwrap_or(0),
            roles: g.map(|g| g.roles.clone()).unwrap_or_default(),
            matches: g.map(|g| g.matches.clone()).unwrap_or_default(),
            bm25_score: b.map(|b| b.score).unwrap_or(0.0),
        }
    }).collect();

    merged.sort_by(|a, b| b.combined_score().partial_cmp(&a.combined_score()).unwrap_or(std::cmp::Ordering::Equal));
    merged
}

fn build_tags(hits: &[grep::GrepHit], _bm25: Option<&[bm25::Bm25Score]>, root: &std::path::Path) -> Vec<Tag> {
    let mut tags = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let max = if hits.len() > 20 { 20 } else { hits.len() };
    for hit in hits.iter().take(max) {
        let rel = hit.file.strip_prefix(root.to_str().unwrap_or("")).unwrap_or(&hit.file);
        let rel = rel.trim_start_matches('/');
        for m in &hit.matches {
            let key = (rel.to_string(), m.name.clone());
            if seen.insert(key) {
                tags.push(Tag {
                    rel_fname: rel.to_string(),
                    fname: hit.file.clone(),
                    line: m.line,
                    name: m.name.clone(),
                    kind: m.role.clone(),
                    score: hit.score,
                });
            }
        }
    }
    tags
}

fn build_tags_from_merged(merged: &[MergedCandidate], root: &std::path::Path) -> Vec<Tag> {
    let mut tags = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for mc in merged.iter().take(20) {
        let rel = mc.file.strip_prefix(root.to_str().unwrap_or("")).unwrap_or(&mc.file);
        let rel = rel.trim_start_matches('/');
        for m in &mc.matches {
            let key = (rel.to_string(), m.name.clone());
            if seen.insert(key) {
                tags.push(Tag {
                    rel_fname: rel.to_string(),
                    fname: mc.file.clone(),
                    line: m.line,
                    name: m.name.clone(),
                    kind: m.role.clone(),
                    score: mc.combined_score(),
                });
            }
        }
    }
    tags
}
