#!/usr/bin/env python3
"""
genx — generate-context adapter for mapx.

Usage:
    genx "<symbols>" --task "<task>" --root <path>
    genx history [--symbols "<symbols>"]

Calls mapx, extracts code snippets, builds call chain display,
manages a persistent history log, and emits baked markdown context
ready to pipe into a coder LLM.
"""

import argparse
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

MAPX_BIN = os.environ.get("MAPX_BIN", "mapx")
DEFAULT_HISTORY = os.path.expanduser("~/.genx_history.md")
DEFAULT_WINDOW_TOKENS = 32_000
SNIPPET_RADIUS = 20          # lines before/after a tag line
TOKEN_ESTIMATE_CHARS = 4     # ~4 chars per token
COMPRESS_THRESHOLD = 0.80    # compress when history > 80% of window
OLLAMA_URL = os.environ.get("OLLAMA_URL", "http://localhost:11434")

# ---------------------------------------------------------------------------
# mapx integration
# ---------------------------------------------------------------------------

def run_mapx(root: str, query: str, call_graph: bool = False) -> dict:
    """Run mapx and return parsed JSON result."""
    cmd = [MAPX_BIN, "--root", root, "--query", query, "--format", "json"]
    if call_graph:
        cmd.append("--call-graph")
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    except FileNotFoundError:
        sys.exit(f"[genx] error: mapx binary not found. Set MAPX_BIN env var or install mapx.")
    except subprocess.TimeoutExpired:
        sys.exit("[genx] error: mapx timed out after 120s")

    if result.returncode != 0:
        sys.exit(f"[genx] error: mapx exited {result.returncode}\n{result.stderr}")

    stdout = result.stdout.strip()
    if not stdout:
        return {"tags": [], "call_graph": None}

    try:
        data = json.loads(stdout)
    except json.JSONDecodeError as e:
        sys.exit(f"[genx] error: could not parse mapx output: {e}\n{stdout[:200]}")

    # Normalise: mapx now returns {"tags": [...], "call_graph": [...]}
    # but be defensive in case someone uses an older binary that returns [...]
    if isinstance(data, list):
        return {"tags": data, "call_graph": None}
    return data

# ---------------------------------------------------------------------------
# Snippet extraction
# ---------------------------------------------------------------------------

def read_snippet(fname: str, line: int, radius: int = SNIPPET_RADIUS) -> str:
    """Read ±radius lines around `line` (1-based) from file."""
    try:
        with open(fname, encoding="utf-8", errors="replace") as f:
            lines = f.readlines()
    except OSError:
        return f"  <could not read {fname}>"

    start = max(0, line - 1 - radius)
    end = min(len(lines), line - 1 + radius + 1)
    snippet_lines = lines[start:end]

    # Add a marker on the target line
    target_idx = (line - 1) - start
    if 0 <= target_idx < len(snippet_lines):
        snippet_lines[target_idx] = "▶ " + snippet_lines[target_idx]

    return "".join(snippet_lines).rstrip()

# ---------------------------------------------------------------------------
# Call chain formatting
# ---------------------------------------------------------------------------

def format_call_chain(call_graph: list[dict] | None, query_symbols: list[str]) -> str:
    """Build a compact call chain string from call_graph edges."""
    if not call_graph:
        return ""

    # Index edges by caller
    by_caller: dict[str, list[str]] = {}
    for edge in call_graph:
        caller = edge.get("caller", "")
        callee = edge.get("callee", "")
        if caller and callee:
            by_caller.setdefault(caller, []).append(callee)

    lines = []
    for sym in query_symbols:
        # Case-insensitive match
        matched = [k for k in by_caller if k.lower() == sym.lower()]
        for caller in matched:
            callees = by_caller[caller]
            lines.append(f"{caller} → " + ", ".join(callees[:8]))
            if len(callees) > 8:
                lines[-1] += f" … (+{len(callees) - 8} more)"

    return "\n".join(lines)

# ---------------------------------------------------------------------------
# Context assembly
# ---------------------------------------------------------------------------

def assemble_context(tags: list[dict], call_graph: list[dict] | None,
                     query: str, task: str, root: str) -> str:
    """Build a baked markdown context section from mapx output."""
    symbols = extract_symbols_from_query(query)
    call_chain = format_call_chain(call_graph, symbols)

    lines = []
    lines.append(f"## Context: {query}")
    if task:
        lines.append(f"**Task**: {task}")
    if call_chain:
        lines.append(f"**Call chain**:")
        for cl in call_chain.splitlines():
            lines.append(f"  {cl}")
    lines.append("")

    # Group tags by file, keep highest-score entry per (file, name) pair
    seen: set[tuple[str, str]] = set()
    by_file: dict[str, list[dict]] = {}
    for tag in sorted(tags, key=lambda t: -t.get("score", 0)):
        key = (tag["rel_fname"], tag["name"])
        if key in seen:
            continue
        seen.add(key)
        by_file.setdefault(tag["rel_fname"], []).append(tag)

    # Determine file language for code fences
    ext_lang = {
        "php": "php", "phtml": "php",
        "js": "javascript", "jsx": "javascript", "mjs": "javascript",
        "ts": "typescript", "tsx": "typescript",
        "rs": "rust", "py": "python",
        "go": "go", "rb": "ruby", "java": "java",
    }

    for rel_fname, file_tags in by_file.items():
        # Merge all hit lines for this file, expanding to a contiguous window
        hit_lines = sorted(set(t["line"] for t in file_tags))
        roles = sorted(set(t["kind"] for t in file_tags))
        score = max(t.get("score", 0) for t in file_tags)
        names = ", ".join(sorted(set(t["name"] for t in file_tags)))

        ext = rel_fname.rsplit(".", 1)[-1].lower() if "." in rel_fname else ""
        lang = ext_lang.get(ext, "")

        lines.append(f"### {rel_fname}")
        lines.append(f"> {', '.join(roles)} | score: {score:.0f} | symbols: {names}")

        # Build a merged snippet covering all hit lines with SNIPPET_RADIUS each
        fname = file_tags[0].get("fname", os.path.join(root, rel_fname))
        try:
            with open(fname, encoding="utf-8", errors="replace") as f:
                all_lines = f.readlines()
        except OSError:
            lines.append(f"```\n<could not read file>\n```\n")
            continue

        # Collect line ranges
        intervals: list[tuple[int, int]] = []
        for hl in hit_lines:
            s = max(0, hl - 1 - SNIPPET_RADIUS)
            e = min(len(all_lines), hl - 1 + SNIPPET_RADIUS + 1)
            intervals.append((s, e))

        # Merge overlapping intervals
        merged: list[tuple[int, int]] = []
        for s, e in sorted(intervals):
            if merged and s <= merged[-1][1]:
                merged[-1] = (merged[-1][0], max(merged[-1][1], e))
            else:
                merged.append((s, e))

        snippets = []
        for s, e in merged:
            chunk = all_lines[s:e]
            # Mark target lines
            for hl in hit_lines:
                idx = (hl - 1) - s
                if 0 <= idx < len(chunk) and not chunk[idx].startswith("▶ "):
                    chunk[idx] = "▶ " + chunk[idx]
            snippets.append("".join(chunk).rstrip())

        snippet_text = "\n\n… (gap) …\n\n".join(snippets)
        lines.append(f"```{lang}")
        lines.append(snippet_text)
        lines.append("```")
        lines.append("")

    return "\n".join(lines)

# ---------------------------------------------------------------------------
# Symbol extraction (mirrors mapx heuristics, lightweight)
# ---------------------------------------------------------------------------

_NOISE = frozenset([
    "the", "this", "that", "with", "from", "into", "onto", "over", "under",
    "what", "when", "where", "which", "while", "about", "after", "before",
    "does", "show", "find", "look", "give", "make", "call", "calls",
    "function", "method", "class", "variable", "symbol", "code", "file",
    "how", "why", "who", "can", "will", "should", "would", "could", "have",
    "and", "for", "not", "but", "all", "any", "one", "its", "add", "use",
])

def extract_symbols_from_query(query: str) -> list[str]:
    """Extract likely code symbol names from a free-text query."""
    # Split on common delimiters
    tokens = re.split(r'[\s,;|/\\]+', query)
    symbols = []
    seen: set[str] = set()
    for tok in tokens:
        # Strip punctuation
        tok = tok.strip("\"'`()[]{}.:!?")
        if len(tok) < 2:
            continue
        # Must look like a code identifier
        if not re.match(r'^[a-zA-Z_][a-zA-Z0-9_]*$', tok):
            continue
        lower = tok.lower()
        if lower in _NOISE:
            continue
        if tok not in seen:
            seen.add(tok)
            symbols.append(tok)
    return symbols[:8]

# ---------------------------------------------------------------------------
# History management
# ---------------------------------------------------------------------------

def estimate_tokens(text: str) -> int:
    return len(text) // TOKEN_ESTIMATE_CHARS

def load_history(path: str) -> str:
    if not os.path.exists(path):
        return ""
    with open(path, encoding="utf-8") as f:
        return f.read()

def save_history(path: str, text: str) -> None:
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)

def append_history(path: str, section: str) -> None:
    with open(path, "a", encoding="utf-8") as f:
        f.write(section)

def compress_history(history: str, model: str) -> str:
    """Ask the LLM to summarize history prose while keeping all facts."""
    prompt = (
        "You are compressing a code context history log.\n"
        "Rules:\n"
        "- Keep ALL file paths, line numbers, symbol names, function names, and call chains exactly as-is.\n"
        "- Compress or remove verbose prose and repeated explanations.\n"
        "- Preserve the markdown structure (## headers, ### subheaders, code blocks).\n"
        "- Do NOT invent new information.\n\n"
        "History to compress:\n\n"
        + history
    )
    return ollama_generate(prompt, model)

def filter_history_by_symbols(history: str, symbols: list[str]) -> str:
    """Return only sections from history that mention any of the given symbols."""
    if not symbols or not history:
        return history

    sections = re.split(r'\n(?=---\n## )', history)
    matching = []
    pattern = re.compile(
        "|".join(re.escape(s) for s in symbols),
        re.IGNORECASE
    )
    for section in sections:
        if pattern.search(section):
            matching.append(section)
    return "\n".join(matching) if matching else history

# ---------------------------------------------------------------------------
# Ollama integration
# ---------------------------------------------------------------------------

def ollama_generate(prompt: str, model: str) -> str:
    """Call Ollama /api/generate and return the response text."""
    try:
        import urllib.request
        import urllib.error
    except ImportError:
        return prompt  # fallback: no-op

    payload = json.dumps({
        "model": model,
        "prompt": prompt,
        "stream": False,
    }).encode()

    req = urllib.request.Request(
        f"{OLLAMA_URL}/api/generate",
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            data = json.loads(resp.read())
            return data.get("response", "").strip()
    except Exception as e:
        print(f"[genx] warning: LLM call failed: {e}", file=sys.stderr)
        return ""

# ---------------------------------------------------------------------------
# Main commands
# ---------------------------------------------------------------------------

def cmd_ctx(args: argparse.Namespace) -> None:
    query = args.query
    task = args.task or ""
    root = str(Path(args.root).resolve())
    history_path = args.history
    window = args.window
    model = args.model
    compress_model = args.compress_model or model

    # 1. Run mapx with call-graph
    print("[genx] running mapx…", file=sys.stderr)
    data = run_mapx(root, query, call_graph=True)
    tags: list[dict] = data.get("tags") or []
    call_graph: list[dict] | None = data.get("call_graph")

    if not tags:
        print("[genx] no results from mapx", file=sys.stderr)
        sys.exit(0)

    # 2. Assemble context section
    context_section = assemble_context(tags, call_graph, query, task, root)

    # 3. Build history entry
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S")
    symbols = extract_symbols_from_query(query)
    symbol_str = ", ".join(symbols)
    hit_files = ", ".join(sorted(set(t["rel_fname"] for t in tags[:10])))

    history_entry = (
        f"\n---\n"
        f"## [{timestamp}] {symbol_str}\n"
        f"**Task**: {task}\n"
        f"**Root**: {root}\n"
        f"**Files hit**: {hit_files}\n\n"
        f"{context_section}\n"
    )

    # 4. Load existing history, check token budget
    history = load_history(history_path)
    history_tokens = estimate_tokens(history)
    window_tokens = window

    if history_tokens > 0 and model:
        usage_ratio = history_tokens / window_tokens
        if usage_ratio >= COMPRESS_THRESHOLD:
            print(
                f"[genx] history at {usage_ratio:.0%} of window ({history_tokens:,} tokens) — compressing…",
                file=sys.stderr
            )
            compressed = compress_history(history, compress_model)
            if compressed:
                history = compressed
                save_history(history_path, history)
                print("[genx] history compressed and rewritten", file=sys.stderr)

    # 5. Append new entry to history file
    append_history(history_path, history_entry)
    print(f"[genx] appended to {history_path}", file=sys.stderr)

    # 6. Decide what to send to stdout
    prior_symbols_found = bool(filter_history_by_symbols(history, symbols).strip()) if history else False

    if prior_symbols_found:
        # Send only matching history sections + current run
        relevant_history = filter_history_by_symbols(history, symbols)
        body = relevant_history.strip() + "\n\n" + history_entry.strip()
    else:
        # First time seeing these symbols — send full history for maximum context
        body = (history.strip() + "\n\n" + history_entry.strip()).strip()

    # Prepend a framing preamble so the coder LLM understands the document structure.
    task_line = task or "(no task specified)"
    symbol_display = ", ".join(symbols) if symbols else query
    preamble = f"""\
You are a senior software engineer. Read the document below carefully before responding.

## How to read this document

**History sections** (entries with ISO timestamps in headings): prior coding sessions on this
codebase. They show what was explored, what decisions were made, and what files were touched.
Use them as background — do not treat them as the current task.

**Current context section** (the last entry, timestamp {timestamp}): fresh code snippets
fetched by mapx for the symbol(s): `{symbol_display}`.
Lines marked with `▶` are the exact matched lines. All other lines are surrounding context.

## Your task

{task_line}

## Rules

- Every claim about code MUST cite the exact file and line number shown in the context.
- If a file or line is not in the context, say so clearly — do not invent paths or numbers.
- Prefer minimal, targeted changes. Do not refactor unrelated code.
- If you produce edits, use SEARCH/REPLACE blocks:
      path/to/file.ext
      SEARCH
      <exact existing lines>
      REPLACE
      <new lines>
- If you need to run a shell command, wrap it in a ```bash fence and it will be executed.
- If the context is insufficient, output on its own line:
      REQUERY <symbol_or_symbols>
  and the pipeline will fetch more context for those symbols.

---

"""
    print(preamble + body)


def cmd_history(args: argparse.Namespace) -> None:
    history = load_history(args.history)
    if not history:
        print(f"[genx] no history at {args.history}")
        return

    if args.symbols:
        symbols = extract_symbols_from_query(args.symbols)
        history = filter_history_by_symbols(history, symbols)

    print(history)


# ---------------------------------------------------------------------------
# CLI entry point
# ---------------------------------------------------------------------------

def main() -> None:
    # If the first argument is not a known subcommand, prepend 'ctx' so the
    # user can write:  genx "symbols" --task "..." --root ...
    known_subcmds = {"ctx", "history", "-h", "--help"}
    if len(sys.argv) > 1 and sys.argv[1] not in known_subcmds:
        sys.argv.insert(1, "ctx")

    parser = argparse.ArgumentParser(
        prog="genx",
        description="generate-context: mapx → call-graph → history → coder LLM",
    )
    subparsers = parser.add_subparsers(dest="command")

    # ctx subcommand
    ctx_parser = subparsers.add_parser("ctx", help="Generate context from symbols")
    _add_ctx_args(ctx_parser)

    # history subcommand
    hist_parser = subparsers.add_parser("history", help="Print history log")
    hist_parser.add_argument("--symbols", default="", help="Filter by symbol names")
    hist_parser.add_argument(
        "--history", default=DEFAULT_HISTORY,
        help=f"History file path (default: {DEFAULT_HISTORY})"
    )

    args = parser.parse_args()

    if args.command == "history":
        cmd_history(args)
    elif args.command == "ctx":
        if not args.query:
            ctx_parser.error("query is required")
        cmd_ctx(args)
    else:
        parser.print_help()


def _add_ctx_args(p: argparse.ArgumentParser) -> None:
    p.add_argument("query", help="Symbol names to look up")
    p.add_argument("--task", default="", help="Description of the coding task")
    p.add_argument("--root", default=".", help="Project root (passed to mapx)")
    p.add_argument(
        "--history", default=DEFAULT_HISTORY,
        help=f"History markdown file (default: {DEFAULT_HISTORY})"
    )
    p.add_argument(
        "--model", default="",
        help="Ollama model for history compression (e.g. qwen3-coder)"
    )
    p.add_argument(
        "--compress-model", default="",
        dest="compress_model",
        help="Override model used for compression (defaults to --model)"
    )
    p.add_argument(
        "--window", type=int, default=DEFAULT_WINDOW_TOKENS,
        help=f"Coder LLM context window in tokens (default: {DEFAULT_WINDOW_TOKENS})"
    )


if __name__ == "__main__":
    main()
