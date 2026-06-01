# mapx

Fast code context mapper for LLM coding tools (Rust CLI).

Pipeline: smart grep + BM25 + graph BFS → ranked symbol tags + caller→callee call graph.

---

## Quickstart

```bash
# 1. Build and install
cargo install --path .

# 2. Find a symbol across your project
mapx --root ~/projects/myapp --query "fullReIndex" --format json

# 3. Include caller→callee call graph in the output
mapx --root ~/projects/myapp --query "fullReIndex" --call-graph --format json

# 4. Use genx to turn mapx output into baked LLM context with history
python3 genx/main.py "fullReIndex" \
  --task "add live index update" \
  --root ~/projects/myapp \
  --model qwen3-coder
```

Output (without `--call-graph`) is a JSON array:
```json
[{"rel_fname": "src/Foo.php", "fname": "/abs/src/Foo.php", "line": 42,
  "name": "fullReIndex", "kind": "function-def", "score": 2004.0}, ...]
```

With `--call-graph`, the output is a JSON object:
```json
{
  "tags": [...],
  "call_graph": [
    {"caller": "fullReIndex", "callee": "buildSemanticIndex",
     "caller_file": "/abs/src/Foo.php", "caller_line": 1336}
  ]
}
```

---

## Install

```bash
cargo install --path .
```

No setup needed — language configs (PHP, JS/TS, Python, Rust) are embedded in the binary.

---

## mapx — symbol search

```bash
mapx --root /path/to/project --query "symbolName" --format json
```

| Flag | Default | Description |
|------|---------|-------------|
| `--root` | `.` | Project root to search |
| `--query` | (required) | Search query — symbol names, natural language, or mixed |
| `--mode` | `full` | Pipeline mode: `grep`, `bm25`, or `full` |
| `--format` | `json` | Output format: `json` or `lines` |
| `--max` | `20` | Max results |
| `--call-graph` | — | Include caller→callee edges in output |
| `--ranker` | — | Ollama model for LLM re-rank |
| `--lang-dir` | — | Path to external language TOML configs |
| `--ollama-base` | `http://localhost:11434` | Ollama server URL |

The spinner is printed to stderr — pipe cleanly with `2>/dev/null`.

### Pipeline stages

1. **Symbol extraction** — parse query into code identifiers (camelCase, PascalCase, dotted paths)
2. **Grep** — match identifiers against per-language regex patterns; score by role (definition = 10×, mention = 1×)
3. **BM25** — re-score files by term frequency across the grep result set
4. **Graph BFS** — expand from seed symbols to depth 2, surface related definitions
5. **Call graph** *(optional)* — brace-depth body scan to extract caller→callee edges
6. **LLM re-rank** *(optional)* — Ollama model scores and re-orders tags

Tags with `score < 1.0` are filtered from all output.

---

## genx — generate context

`genx` wraps mapx to produce baked markdown context for a coder LLM, with persistent history.

```bash
python3 genx/main.py "fullReIndex, PortfolioIq" \
  --task "add live index update" \
  --root ~/projects/myapp \
  --model qwen3-coder \
  --history ~/.genx_history.md
```

| Flag | Default | Description |
|------|---------|-------------|
| `query` | (required) | Symbol names to look up |
| `--task` | — | Coding task description (included in context and history) |
| `--root` | `.` | Project root passed to mapx |
| `--history` | `~/.genx_history.md` | Append-only context history file |
| `--model` | — | Ollama model for history compression |
| `--compress-model` | `--model` | Override model used for compression only |
| `--window` | `32000` | Coder LLM context window in tokens |

**What genx does:**

1. Runs `mapx --call-graph` → ranked tags + call edges
2. Reads ±20 lines around each tag location, grouped by file
3. Emits a baked markdown context section with code snippets, call chain, and scores
4. Appends the run to `--history` with a timestamp
5. When history exceeds 80% of `--window`, calls `--model` to compress it (preserving all file paths, line numbers, and symbol names)
6. On repeat queries for the same symbols, outputs only matching history sections + the current run

**View history:**
```bash
python3 genx/main.py history --symbols "fullReIndex"
```

**Pipe to a coder LLM:**
```bash
python3 genx/main.py "fullReIndex" --task "add live index" --root ~/projects/myapp \
  2>/dev/null | ollama run qwen3-coder
```

---

## Language configs

Languages are embedded at build time. Override or add languages by creating a TOML file:

```toml
extensions = [".rs"]

[[patterns]]
role = "function-def"
priority = 100
regex = "(?:pub\\s+)?(?:unsafe\\s+)?fn\\s+{T}\\s*\\("
```

`{T}` is replaced with the escaped query term at runtime. Supported roles and their score multipliers:

| Role | Multiplier |
|------|-----------|
| `definition`, `function-def`, `interface`, `trait`, `type` | 10× |
| `import`, `variable`, `extends`, `implements` | 4× |
| `static-call`, `instantiation`, `method-call` | 3× |
| `type-hint`, `type-ref`, `member-access` | 2× |
| `docblock`, `decorator`, `macro` | 1.5× |
| `mention` | 1× |

Place custom configs in `languages/` in the project root, or pass `--lang-dir`.

---

## Tests

```bash
cargo test
```
