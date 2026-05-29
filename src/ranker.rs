use crate::Tag;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct RankItem {
    file: String,
    score: u32,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

fn build_prompt(query: &str, tags: &[Tag]) -> String {
    let mut lines = vec![
        "You are scoring code files for relevance to a developer question.".to_string(),
        "".to_string(),
        format!("Question: \"{query}\""),
        "".to_string(),
        "Rate each file 0-10 for relevance:".to_string(),
        "  10 = contains the EXACT definition/declaration of the main symbol".to_string(),
        "  7-9 = important usages, callers, or imports of the symbol".to_string(),
        "  4-6 = mentions the symbol but indirectly relevant".to_string(),
        "  1-3 = incidental mentions (comments, unrelated code)".to_string(),
        "  0 = completely irrelevant".to_string(),
        "".to_string(),
        "Return ONLY a JSON array sorted by score descending:".to_string(),
        "[{ \"file\": \"...\", \"score\": 0-10, \"reason\": \"short reason\" }]".to_string(),
        "".to_string(),
        "Candidates:".to_string(),
    ];

    for (i, tag) in tags.iter().enumerate() {
        lines.push(format!("[{}] {}:{} {} ({})", i + 1, tag.rel_fname, tag.line, tag.name, tag.kind));
    }

    lines.join("\n")
}

pub fn llm_rank(tags: &[Tag], model: &str, ollama_base: &str) -> Vec<Tag> {
    let prompt = build_prompt("ranking", tags);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .ok();

    let client = match client {
        Some(c) => c,
        None => return tags.to_vec(),
    };

    let body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "options": { "num_predict": 2000 }
    });

    let url = format!("{}/api/generate", ollama_base.trim_end_matches('/'));

    let resp = match client.post(&url).json(&body).send() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[mapx] LLM rank request failed: {e}");
            return tags.to_vec();
        }
    };

    let data: OllamaResponse = match resp.json() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[mapx] LLM rank parse failed: {e}");
            return tags.to_vec();
        }
    };

    let parsed: Vec<RankItem> = match serde_json::from_str(&data.response) {
        Ok(items) => items,
        Err(_) => {
            // Try to extract JSON array from response
            let re = regex::Regex::new(r"\[[\s\S]*\]").ok();
            if let Some(re) = re {
                if let Some(m) = re.find(&data.response) {
                    if let Ok(items) = serde_json::from_str::<Vec<RankItem>>(m.as_str()) {
                        return reorder_tags(tags, &items);
                    }
                }
            }
            return tags.to_vec();
        }
    };

    reorder_tags(tags, &parsed)
}

fn reorder_tags(tags: &[Tag], ranked: &[RankItem]) -> Vec<Tag> {
    let mut result: Vec<Tag> = tags.to_vec();

    // Apply score from ranking to matching tags
    for tag in &mut result {
        if let Some(rank_item) = ranked.iter().find(|r| {
            let r_file = r.file.replace("\\", "/");
            let t_file = tag.rel_fname.replace("\\", "/");
            t_file.ends_with(&r_file) || r_file.ends_with(&t_file)
        }) {
            tag.score = rank_item.score as f64;
        }
    }

    result.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    result
}
