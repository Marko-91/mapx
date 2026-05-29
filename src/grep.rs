use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::Deserialize;
use regex::Regex;
use walkdir::WalkDir;

/// Embedded language configs — no external files needed at runtime.
const EMBEDDED_LANGS: &[(&str, &str)] = &[
    ("js",    include_str!("../languages/js.toml")),
    ("php",   include_str!("../languages/php.toml")),
    ("python", include_str!("../languages/python.toml")),
    ("rust",  include_str!("../languages/rust.toml")),
];

#[derive(Debug, Clone, Deserialize)]
pub struct LanguageConfig {
    pub extensions: Vec<String>,
    pub patterns: Vec<PatternDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PatternDef {
    pub role: String,
    pub priority: u32,
    pub regex: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MatchInfo {
    pub line: usize,
    pub priority: u32,
    pub role: String,
    pub match_line: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct GrepHit {
    pub file: String,
    pub score: f64,
    pub best_priority: u32,
    pub roles: Vec<String>,
    pub matches: Vec<MatchInfo>,
}

static ROLE_MULTIPLIERS: &[(&str, f64)] = &[
    ("definition", 10.0),
    ("interface", 10.0),
    ("function-def", 10.0),
    ("type", 10.0),
    ("trait", 10.0),
    ("import", 4.0),
    ("variable", 4.0),
    ("extends", 4.0),
    ("implements", 4.0),
    ("static-call", 3.0),
    ("instantiation", 3.0),
    ("method-call", 3.0),
    ("type-hint", 2.0),
    ("type-ref", 2.0),
    ("member-access", 2.0),
    ("docblock", 1.5),
    ("jsdoc", 1.5),
    ("docstring-ref", 1.5),
    ("decorator", 1.5),
    ("macro", 1.5),
    ("derive", 1.5),
    ("impl", 1.5),
    ("lifetime", 1.5),
    ("namespace", 1.5),
    ("use-trait", 1.0),
    ("mention", 1.0),
];

fn role_multiplier(role: &str) -> f64 {
    for (r, m) in ROLE_MULTIPLIERS {
        if *r == role {
            return *m;
        }
    }
    1.0
}

fn compile_pattern(raw: &str, term: &str) -> Option<Regex> {
    let escaped = regex::escape(term);
    let pattern = raw.replace("{T}", &escaped);
    Regex::new(&pattern).ok()
}

fn load_embedded() -> HashMap<String, LanguageConfig> {
    let mut configs = HashMap::new();
    for (name, content) in EMBEDDED_LANGS {
        if let Ok(config) = toml::from_str::<LanguageConfig>(content) {
            configs.insert(name.to_string(), config);
        }
    }
    configs
}

fn load_external(root: &Path, lang_dir: Option<&Path>) -> HashMap<String, LanguageConfig> {
    let mut configs = HashMap::new();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(ld) = lang_dir {
        candidates.push(ld.to_path_buf());
    }
    candidates.push(root.join("languages"));
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("languages"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("languages"));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(home).join(".local").join("share").join("mapx").join("languages"));
    }

    for dir in &candidates {
        if !dir.exists() { continue; }
        if let Ok(read_dir) = std::fs::read_dir(dir) {
            for entry in read_dir.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let lang_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
                        if let Ok(config) = toml::from_str::<LanguageConfig>(&content) {
                            configs.insert(lang_name, config);
                        }
                    }
                }
            }
        }
    }
    configs
}

pub fn load_language_patterns(root: &Path, lang_dir: Option<&Path>) -> Result<HashMap<String, LanguageConfig>, String> {
    let mut configs = load_embedded();
    let external = load_external(root, lang_dir);
    // External configs override embedded (allows users to extend/customize)
    for (k, v) in external {
        configs.insert(k, v);
    }
    if configs.is_empty() {
        eprintln!("[mapx] warning: no language configs found.");
    }
    Ok(configs)
}



pub fn smart_grep(root: &Path, terms: &[String], lang_configs: &HashMap<String, LanguageConfig>) -> Vec<GrepHit> {
    let mut results = Vec::new();

    for (_lang_name, config) in lang_configs {
        let files = find_files(root, &config.extensions);
        if files.is_empty() { continue; }

        for filepath in &files {
            let content = match std::fs::read_to_string(filepath) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let lines: Vec<&str> = content.lines().collect();
            let mut file_matches = Vec::new();

            for term in terms {
                for pat in &config.patterns {
                    let Some(re) = compile_pattern(&pat.regex, term) else { continue };
                    for (i, line) in lines.iter().enumerate().take(2000) {
                        if re.is_match(line) {
                            file_matches.push(MatchInfo {
                                line: i + 1,
                                priority: pat.priority,
                                role: pat.role.clone(),
                                match_line: line.trim().chars().take(200).collect(),
                                name: term.clone(),
                            });
                        }
                    }
                }
            }

            if !file_matches.is_empty() {
                let best_priority = file_matches.iter().map(|m| m.priority).max().unwrap_or(0);
                let roles: Vec<String> = {
                    let mut r: Vec<String> = file_matches.iter().map(|m| m.role.clone()).collect();
                    r.sort();
                    r.dedup();
                    r
                };
                let score: f64 = file_matches.iter().map(|m| m.priority as f64 * role_multiplier(&m.role)).sum();
                results.push(GrepHit {
                    file: filepath.to_string_lossy().to_string(),
                    score,
                    best_priority,
                    roles,
                    matches: file_matches,
                });
            }
        }
    }

    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results
}

fn find_files(root: &Path, extensions: &[String]) -> Vec<std::path::PathBuf> {
    let ext_set: std::collections::HashSet<String> = extensions.iter()
        .map(|e| e.trim_start_matches('.').to_lowercase())
        .collect();

    let ignored: std::collections::HashSet<&str> = [
        "node_modules", "vendor", ".git", "target", "dist", "build",
        ".next", "__pycache__", "venv", ".venv", ".cache", ".vscode",
        ".idea", ".gradle", "bazel-bin", "bazel-out", "bazel-testlogs",
        "coverage", ".github", "tmp", "temp",
    ].iter().copied().collect();

    let mut files = Vec::new();
    for entry in WalkDir::new(root).into_iter().filter_entry(|e| {
        !ignored.contains(e.file_name().to_str().unwrap_or(""))
            && !e.file_name().to_str().map_or(false, |n| n.starts_with('.')) && e.file_name() != ".github"
    }) {
        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                    if ext_set.contains(&ext.to_lowercase()) {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_pattern_valid() {
        let re = compile_pattern(r"function\s+{T}\s*\(", "buildMiniGraph");
        assert!(re.is_some());
        let re = re.unwrap();
        assert!(re.is_match("function buildMiniGraph("));
        assert!(re.is_match("async function buildMiniGraph("));
        assert!(!re.is_match("function buildMiniGraphExtra("));
    }

    #[test]
    fn test_compile_pattern_escaping() {
        // regex::escape("Container::make") = "Container::make" (colons are not special)
        let re = compile_pattern(r"\b{T}\b", "Container::make");
        assert!(re.is_some());
        let re = re.unwrap();
        assert!(re.is_match("Container::make"));
    }

    #[test]
    fn test_role_multiplier() {
        assert_eq!(role_multiplier("definition"), 10.0);
        assert_eq!(role_multiplier("function-def"), 10.0);
        assert_eq!(role_multiplier("import"), 4.0);
        assert_eq!(role_multiplier("mention"), 1.0);
        assert_eq!(role_multiplier("unknown"), 1.0);
    }
}
