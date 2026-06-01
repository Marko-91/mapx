use regex::Regex;
use serde::Serialize;

use crate::grep::{GrepHit, MatchInfo};

/// A single caller → callee edge found inside a function body.
#[derive(Debug, Clone, Serialize)]
pub struct CallEdge {
    pub caller: String,
    pub callee: String,
    pub caller_file: String,
    pub caller_line: usize,
}

// Regex patterns for function-definition lines (to detect body start)
// and method/function call patterns (to detect callees inside a body).
//
// Each entry: (language extensions hint, def_regex, call_regex)
// `{NAME}` in def_regex is replaced by the caller function name at match time.
// call_regex extracts the callee name from a call site line.

struct LangCallPattern {
    /// File extension (without dot)
    exts: &'static [&'static str],
    /// Regex matching a function definition whose name is captured in group 1
    def_re: &'static str,
    /// Regex matching a call site; callee name in group 1
    call_re: &'static str,
    /// Whether to use indentation-based body detection (Python) vs brace-depth
    indent_based: bool,
}

const LANG_PATTERNS: &[LangCallPattern] = &[
    LangCallPattern {
        exts: &["php", "phtml"],
        def_re: r"(?:public|private|protected|static|abstract|final|\s)*function\s+(\w+)\s*\(",
        call_re: r"(?:->|::)(\w+)\s*\(",
        indent_based: false,
    },
    LangCallPattern {
        exts: &["js", "jsx", "ts", "tsx", "mjs", "cjs"],
        def_re: r"(?:async\s+)?function\s+(\w+)\s*\(|(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s*)?\(",
        call_re: r"(?:this\.|self\.)?(\w+)\s*\(",
        indent_based: false,
    },
    LangCallPattern {
        exts: &["rs"],
        def_re: r"(?:pub\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+(\w+)\s*[<(]",
        call_re: r"(\w+)\s*\(",
        indent_based: false,
    },
    LangCallPattern {
        exts: &["py"],
        def_re: r"(?:async\s+)?def\s+(\w+)\s*\(",
        call_re: r"(\w+)\s*\(",
        indent_based: true,
    },
];

/// Noise words to ignore as callee names (language keywords, common builtins)
const CALL_NOISE: &[&str] = &[
    "if", "for", "while", "switch", "match", "catch", "return", "yield",
    "println", "print", "echo", "require", "include", "isset", "empty",
    "array", "list", "die", "exit", "throw", "new", "assert",
    "len", "str", "int", "bool", "vec", "map", "set",
];

fn ext_of(file: &str) -> &str {
    std::path::Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
}

fn lang_pattern_for(file: &str) -> Option<&'static LangCallPattern> {
    let ext = ext_of(file);
    LANG_PATTERNS.iter().find(|p| p.exts.contains(&ext))
}

/// Extract call edges from a single file for every hit function-def.
fn extract_edges_from_file(
    file: &str,
    content: &str,
    def_matches: &[&MatchInfo],
    lang: &LangCallPattern,
) -> Vec<CallEdge> {
    let def_re = match Regex::new(lang.def_re) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let call_re = match Regex::new(lang.call_re) {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut edges = Vec::new();

    for hit_match in def_matches {
        let def_line_idx = hit_match.line.saturating_sub(1); // 0-based

        // Find the actual definition line at or near the hit line
        // (the hit might be the exact line or close to it)
        let search_start = def_line_idx.saturating_sub(2);
        let search_end = (def_line_idx + 3).min(lines.len());

        let mut caller_name: Option<String> = None;
        let mut body_start: Option<usize> = None;

        for i in search_start..search_end {
            if let Some(caps) = def_re.captures(lines[i]) {
                // group 1 or 2 depending on language pattern
                let name = caps.get(1)
                    .or_else(|| caps.get(2))
                    .map(|m| m.as_str().to_string());
                caller_name = name;
                body_start = Some(i);
                break;
            }
        }

        let caller_name = match caller_name {
            Some(n) => n,
            None => hit_match.name.clone(), // fallback: use the query term
        };
        let body_start = body_start.unwrap_or(def_line_idx);

        // Find function body bounds
        let body_end = if lang.indent_based {
            find_body_end_indent(&lines, body_start)
        } else {
            find_body_end_braces(&lines, body_start)
        };

        // Scan body for call sites
        for i in (body_start + 1)..body_end.min(lines.len()) {
            for caps in call_re.captures_iter(lines[i]) {
                let callee = caps.get(1)
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_default();
                if callee.is_empty()
                    || callee == caller_name
                    || CALL_NOISE.contains(&callee.as_str())
                    || callee.len() < 3
                {
                    continue;
                }
                edges.push(CallEdge {
                    caller: caller_name.clone(),
                    callee,
                    caller_file: file.to_string(),
                    caller_line: body_start + 1, // 1-based
                });
            }
        }
    }

    // Deduplicate: same caller+callee pair, keep first occurrence
    let mut seen = std::collections::HashSet::new();
    edges.retain(|e| seen.insert((e.caller.clone(), e.callee.clone())));
    edges
}

/// Find function body end using brace-depth counting (C-style languages).
fn find_body_end_braces(lines: &[&str], def_line: usize) -> usize {
    let mut depth: i32 = 0;
    let mut entered = false;

    for (i, line) in lines.iter().enumerate().skip(def_line) {
        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    entered = true;
                }
                '}' => {
                    depth -= 1;
                    if entered && depth <= 0 {
                        return i + 1;
                    }
                }
                _ => {}
            }
        }
    }
    lines.len()
}

/// Find function body end using indentation (Python).
fn find_body_end_indent(lines: &[&str], def_line: usize) -> usize {
    // Get indentation of the def line
    let def_indent = lines[def_line].len() - lines[def_line].trim_start().len();

    for (i, line) in lines.iter().enumerate().skip(def_line + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue; // blank lines don't end the body
        }
        let indent = line.len() - line.trim_start().len();
        if indent <= def_indent {
            return i;
        }
    }
    lines.len()
}

/// Build call edges for all grep hits that contain function-def matches.
pub fn build_call_graph(hits: &[GrepHit]) -> Vec<CallEdge> {
    let mut all_edges = Vec::new();

    for hit in hits {
        let lang = match lang_pattern_for(&hit.file) {
            Some(l) => l,
            None => continue,
        };

        // Only process files that have at least one function-def match
        let def_matches: Vec<&MatchInfo> = hit.matches.iter()
            .filter(|m| m.role == "function-def" || m.role == "definition")
            .collect();
        if def_matches.is_empty() {
            continue;
        }

        let content = match std::fs::read_to_string(&hit.file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let edges = extract_edges_from_file(&hit.file, &content, &def_matches, lang);
        all_edges.extend(edges);
    }

    all_edges
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brace_body_end() {
        let src = vec![
            "function foo() {",
            "    bar();",
            "    baz();",
            "}",
            "function other() {}",
        ];
        let end = find_body_end_braces(&src, 0);
        assert_eq!(end, 4);
    }

    #[test]
    fn test_indent_body_end() {
        let src = vec![
            "def foo():",
            "    bar()",
            "    baz()",
            "def other():",
            "    pass",
        ];
        let end = find_body_end_indent(&src, 0);
        assert_eq!(end, 3);
    }

    #[test]
    fn test_noise_filtered() {
        assert!(CALL_NOISE.contains(&"if"));
        assert!(!CALL_NOISE.contains(&"fullReIndex"));
    }
}
