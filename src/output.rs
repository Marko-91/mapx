use std::io::{self, Write};
use crate::{Tag, PipelineResult};

pub fn write_json(result: &PipelineResult, writer: &mut impl Write) -> io::Result<()> {
    let json = serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".to_string());
    writeln!(writer, "{json}")
}

pub fn write_lines(tags: &[Tag], writer: &mut impl Write) -> io::Result<()> {
    for tag in tags {
        writeln!(writer, "{}:{} {} ({})", tag.rel_fname, tag.line, tag.name, tag.kind)?;
    }
    Ok(())
}

/// Simple spinner printed to stderr. Uses braille unicode dots.
pub struct Spinner {
    frame: usize,
    label: String,
    done: bool,
}

const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl Spinner {
    pub fn new(label: &str) -> Self {
        let mut s = Spinner {
            frame: 0,
            label: label.to_string(),
            done: false,
        };
        s.tick();
        s
    }

    pub fn tick(&mut self) {
        if self.done { return; }
        let frame = FRAMES[self.frame % FRAMES.len()];
        eprint!("\r{frame} mapx: {}...", self.label);
        self.frame += 1;
        // No flush here — caller should call tick() in a loop
    }

    pub fn done(&mut self) {
        self.done = true;
        eprintln!("\r✓ mapx: {} done", self.label);
    }
}

pub fn with_spinner<F, T>(label: &str, f: F) -> T
where
    F: FnOnce() -> T,
{
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let label_owned = label.to_string();

    let handle = thread::spawn(move || {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut i = 0;
        while r.load(Ordering::Relaxed) {
            eprint!("\r{} mapx: {}...", frames[i % frames.len()], label_owned);
            i += 1;
            thread::sleep(Duration::from_millis(80));
        }
    });

    let result = f();
    running.store(false, Ordering::Relaxed);
    let _ = handle.join();
    eprintln!("\r✓ mapx: {label} done     ");
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PipelineResult;

    #[test]
    fn test_write_json_empty() {
        let result = PipelineResult { tags: vec![], call_graph: None };
        let mut buf = Vec::new();
        write_json(&result, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("\"tags\""));
    }

    #[test]
    fn test_write_json_one_tag() {
        let tags = vec![Tag {
            rel_fname: "src/main.rs".to_string(),
            fname: "/abs/src/main.rs".to_string(),
            line: 10,
            name: "foo".to_string(),
            kind: "def".to_string(),
            score: 1.0,
        }];
        let result = PipelineResult { tags, call_graph: None };
        let mut buf = Vec::new();
        write_json(&result, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("src/main.rs"));
        assert!(output.contains("\"foo\""));
    }

    #[test]
    fn test_write_lines() {
        let tags = vec![Tag {
            rel_fname: "src/main.rs".to_string(),
            fname: "/abs/src/main.rs".to_string(),
            line: 10,
            name: "foo".to_string(),
            kind: "def".to_string(),
            score: 1.0,
        }];
        let mut buf = Vec::new();
        write_lines(&tags, &mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("src/main.rs:10 foo (def)"));
    }
}
