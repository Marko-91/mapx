use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use regex::Regex;

use crate::{MergedCandidate, Tag};

#[derive(Debug, Clone)]
pub struct CodeGraph {
    pub nodes: HashMap<String, NodeInfo>,
    pub adj: HashMap<String, Vec<Edge>>,
    pub rev_adj: HashMap<String, Vec<Edge>>,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub id: String,
    pub node_type: String,
    pub file: String,
    pub name: Option<String>,
    pub kind: Option<String>,
    pub line_range: Option<(usize, usize)>,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub edge_type: String,
    pub line: Option<usize>,
}

impl CodeGraph {
    pub fn new() -> Self {
        CodeGraph {
            nodes: HashMap::new(),
            adj: HashMap::new(),
            rev_adj: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: String, info: NodeInfo) {
        self.nodes.entry(id).or_insert(info);
    }

    pub fn add_edge(&mut self, from: String, to: String, edge_type: String, line: Option<usize>) {
        let edge = Edge { from: from.clone(), to: to.clone(), edge_type, line };
        self.adj.entry(from.clone()).or_default().push(edge.clone());
        self.rev_adj.entry(to).or_default().push(edge);
    }
}

fn extract_symbols(content: &str, file: &str) -> Vec<NodeInfo> {
    let mut symbols = Vec::new();

    let patterns: Vec<(&str, &str)> = vec![
        ("def", r"(?:pub\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+(\w+)\s*(?:<|\(|->)"),
        ("def", r"(?:pub\s+)?(?:struct|enum|trait|union)\s+(\w+)"),
        ("def", r"(?:export\s+)?(?:default\s+)?class\s+(\w+)"),
        ("def", r"(?:export\s+)?(?:async\s+)?function\s+(\w+)"),
        ("def", r"(?:abstract\s+)?(?:final\s+)?class\s+(\w+)"),
        ("def", r"interface\s+(\w+)"),
        ("def", r"trait\s+(\w+)"),
        ("def", r"(?:async\s+)?def\s+(\w+)"),
        ("def", r"class\s+(\w+)"),
        ("def", r"(?:const|let|var)\s+(\w+)\s*[=:(]"),
    ];

    for (kind, pat) in &patterns {
        if let Ok(re) = Regex::new(pat) {
            for cap in re.captures_iter(content) {
                let name = cap[1].to_string();
                let pos = cap.get(1).map(|m| m.start()).unwrap_or(0);
                let line = content[..pos].matches('\n').count() + 1;
                symbols.push(NodeInfo {
                    id: format!("sym://{}:{}", file, name),
                    node_type: "symbol".to_string(),
                    file: file.to_string(),
                    name: Some(name),
                    kind: Some(kind.to_string()),
                    line_range: Some((line, line + 5)),
                });
            }
        }
    }

    symbols
}

fn extract_references(content: &str, file: &str, known_symbols: &[String]) -> Vec<NodeInfo> {
    let mut refs = Vec::new();
    for sym_name in known_symbols {
        let pat = format!(r"\b{}\b", regex::escape(sym_name));
        if let Ok(re) = Regex::new(&pat) {
            for m in re.find_iter(content) {
                let line = content[..m.start()].matches('\n').count() + 1;
                let id = format!("sym://{}:{}", file, sym_name);
                refs.push(NodeInfo {
                    id,
                    node_type: "symbol".to_string(),
                    file: file.to_string(),
                    name: Some(sym_name.clone()),
                    kind: Some("ref".to_string()),
                    line_range: Some((line, line)),
                });
            }
        }
    }
    refs
}

pub fn build_graph_tags(merged: &[MergedCandidate], terms: &[String], root: &Path) -> Vec<Tag> {
    let mut graph = CodeGraph::new();
    let top_files: Vec<&MergedCandidate> = merged.iter().take(10).collect();

    // Read files and build nodes
    for mc in &top_files {
        let content = match std::fs::read_to_string(&mc.file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let rel = mc.file.strip_prefix(root.to_str().unwrap_or("")).unwrap_or(&mc.file);
        let rel = rel.trim_start_matches('/');

        // File node
        let file_node_id = format!("file://{}", mc.file);
        let file_node = NodeInfo {
            id: file_node_id.clone(),
            node_type: "file".to_string(),
            file: mc.file.clone(),
            name: Some(rel.to_string()),
            kind: None,
            line_range: None,
        };
        graph.add_node(file_node_id.clone(), file_node);

        // Symbol nodes
        let symbols = extract_symbols(&content, &mc.file);
        let sym_names: Vec<String> = symbols.iter().filter_map(|s| s.name.clone()).collect();
        for sym in symbols {
            let sym_id = sym.id.clone();
            graph.add_node(sym_id.clone(), sym.clone());
            graph.add_edge(file_node_id.clone(), sym_id, "contains".to_string(), sym.line_range.map(|r| r.0));
        }

        // Reference nodes
        let refs = extract_references(&content, &mc.file, &sym_names);
        for rf in refs {
            graph.add_node(rf.id.clone(), rf.clone());
        }
    }

    // BFS from seed symbols matching query terms
    let seed_ids: Vec<String> = graph.nodes.iter()
        .filter(|(_, n)| {
            n.node_type == "symbol" && n.name.as_ref().map_or(false, |name| {
                terms.iter().any(|t| name.to_lowercase() == t.to_lowercase())
            })
        })
        .take(10)
        .map(|(id, _)| id.clone())
        .collect();

    let bfs_results = bfs(&graph, &seed_ids, 2, 30);

    // Build tags from BFS results + initial matches
    let mut tags = Vec::new();
    let mut seen = HashSet::new();

    // First, add definition tags from merged candidates
    for mc in merged.iter().take(20) {
        let rel = mc.file.strip_prefix(root.to_str().unwrap_or("")).unwrap_or(&mc.file);
        let rel = rel.trim_start_matches('/');
        for m in &mc.matches {
            let key = (rel.to_string(), m.name.clone(), m.line);
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

    // Add BFS-discovered symbols
    for (sym_name, bfs_file, line) in &bfs_results {
        let rel = bfs_file.strip_prefix(root.to_str().unwrap_or("")).unwrap_or(bfs_file);
        let rel = rel.trim_start_matches('/');
        let key = (rel.to_string(), sym_name.clone(), *line);
        if seen.insert(key) {
            tags.push(Tag {
                rel_fname: rel.to_string(),
                fname: bfs_file.clone(),
                line: *line,
                name: sym_name.clone(),
                kind: "def".to_string(),
                score: 0.5,
            });
        }
    }

    tags
}

fn bfs(graph: &CodeGraph, seeds: &[String], max_depth: usize, max_nodes: usize) -> Vec<(String, String, usize)> {
    if seeds.is_empty() {
        return vec![];
    }

    let mut visited = HashSet::new();
    let mut results = Vec::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();

    for seed in seeds {
        queue.push_back((seed.clone(), 0));
    }

    while let Some((id, depth)) = queue.pop_front() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if results.len() >= max_nodes {
            break;
        }

        if let Some(node) = graph.nodes.get(&id) {
            if node.node_type == "symbol" {
                let name = node.name.clone().unwrap_or_default();
                let line = node.line_range.map(|r| r.0).unwrap_or(0);
                results.push((name, node.file.clone(), line));
            }
        }

        if depth >= max_depth {
            continue;
        }

        if let Some(neighbors) = graph.adj.get(&id) {
            for edge in neighbors {
                if !visited.contains(&edge.to) {
                    queue.push_back((edge.to.clone(), depth + 1));
                }
            }
        }
        if let Some(neighbors) = graph.rev_adj.get(&id) {
            for edge in neighbors {
                if !visited.contains(&edge.from) {
                    queue.push_back((edge.from.clone(), depth + 1));
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_fn_def() {
        let content = "pub fn buildMiniGraph() { }".to_string();
        let symbols = extract_symbols(&content, "test.rs");
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.name.as_deref() == Some("buildMiniGraph")));
    }

    #[test]
    fn test_extract_class_def() {
        let content = "export class CodeGraph { }".to_string();
        let symbols = extract_symbols(&content, "test.js");
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.name.as_deref() == Some("CodeGraph")));
    }

    #[test]
    fn test_bfs_empty() {
        let graph = CodeGraph::new();
        let result = bfs(&graph, &[], 2, 10);
        assert!(result.is_empty());
    }

    #[test]
    fn test_bfs_depth_limit() {
        let mut graph = CodeGraph::new();
        graph.add_node("a".to_string(), NodeInfo {
            id: "a".to_string(), node_type: "symbol".to_string(),
            file: "f.rs".to_string(), name: Some("A".to_string()),
            kind: Some("def".to_string()), line_range: Some((1, 1)),
        });
        graph.add_node("b".to_string(), NodeInfo {
            id: "b".to_string(), node_type: "symbol".to_string(),
            file: "f.rs".to_string(), name: Some("B".to_string()),
            kind: Some("def".to_string()), line_range: Some((2, 2)),
        });
        graph.add_edge("a".to_string(), "b".to_string(), "calls".to_string(), Some(2));

        let result = bfs(&graph, &["a".to_string()], 1, 10);
        // Should include both A (depth 0) and B (depth 1)
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|(n, _, _)| n == "A"));
        assert!(result.iter().any(|(n, _, _)| n == "B"));
    }

    #[test]
    fn test_extract_python_def() {
        let content = "async def lazyBuildContext(): pass".to_string();
        let symbols = extract_symbols(&content, "test.py");
        assert!(!symbols.is_empty());
        assert!(symbols.iter().any(|s| s.name.as_deref() == Some("lazyBuildContext")));
    }
}
