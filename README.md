# mapx

Fast code context mapper for code aid tools (Rust CLI).

Pipeline: smart grep + BM25 + graph BFS → ranked tags.

## Install

```bash
cargo install --path .
```

After install, copy the language configs so mapx can find them:

```bash
mkdir -p ~/.local/share/mapx/languages
cp languages/*.toml ~/.local/share/mapx/languages/
```

## Usage

```bash
mapx --root /path/to/project --query "symbolName" --format json
```

Flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--root` | `.` | Project root to search |
| `--query` | (required) | Search query |
| `--mode` | `full` | Pipeline mode: `grep`, `bm25`, or `full` |
| `--format` | `json` | Output format: `json` or `lines` |
| `--max` | `20` | Max results |
| `--ranker` | — | Ollama model for LLM re-rank |
| `--lang-dir` | — | Path to language TOML configs |
| `--ollama-base` | `http://localhost:11434` | Ollama server URL |

## Vanilla Ollama

Pipe mapx output into `ollama run` for LLM re-ranking:

```bash
# Lines format — good for small contexts
CANDIDATES=$(mapx --root src --lang-dir ~/projects/mapx/languages \
  --query "buildMiniGraph" --format lines 2>/dev/null)

ollama run deepseek-r1:7b "Rank these files by relevance to 'buildMiniGraph':
$CANDIDATES
Return only file paths, highest relevance first."
```

Or inline without a temp variable:

```bash
mapx --root src --lang-dir ~/projects/mapx/languages \
  --query "buildMiniGraph" --format json 2>/dev/null \
  | ollama run deepseek-r1:7b '
Rank these code files by relevance to "buildMiniGraph".
Return only file paths, one per line, highest relevance first.
'
```

The spinner goes to stderr (`2>/dev/null` silences it); the actual data goes to stdout, so piping works cleanly.

## Language configs

Add new languages by creating a TOML file in the `languages/` directory:

```toml
extensions = [".rs"]

[[patterns]]
role = "function-def"
priority = 100
regex = "(?:pub\\s+)?(?:unsafe\\s+)?fn\\s+{T}\\s*<"
```

## Tests

```bash
cargo test
```
