# CLAUDE.md

Guidance for Claude Code (and human contributors) when working in this repository.

## Project overview

**ClaideInfo** is a small Rust CLI that reports how much you have used Claude Code,
"Steam playtime" style. It scans the local Claude Code session logs and prints, summed
across every session, the total time Claude worked and the number of tokens spent —
both as an overall total and broken down per project.

It reads the JSONL session files under `~/.claude/projects/*/*.jsonl`:
- **Worked time** comes from `system` events with `subtype == "turn_duration"` (`durationMs`).
- **Tokens** come from `assistant` messages' `usage` object (input + output + cache read +
  cache creation), deduplicated globally by `message.id` so each API call is counted once.
- Files directly under a project dir are real **sessions**; nested files (e.g.
  `<sessionId>/subagents/*.jsonl`) still count time/tokens but do not inflate the session count.

## Build & run

```bash
cargo build --release      # binary name is `claude-info` (see [[bin]] in Cargo.toml)
cargo run --release        # build and run
./target/release/claude-info
```

The tool takes no arguments; it requires the `HOME` environment variable and an existing
`~/.claude/projects` directory.

## Architecture

Single binary; all logic lives in `src/main.rs`:
- `main` — resolves `~/.claude/projects`, walks each project directory, aggregates into a
  `HashMap<String, Stats>`, then prints the report.
- `collect_jsonl` — recursively gathers `.jsonl` files, tagging top-level ones as sessions.
- `scan_session` — parses a file line by line, accumulating time/tokens with global dedup.
- `usage_total` — sums the four token kinds of a `usage` object.
- `print_report` — renders the overall total and the per-project ranking (sorted by time).

## Conventions

These are the rules for all contributions to this open-source project.

### Language
- **English everywhere.** Code, identifiers, comments, doc-comments, commit messages, and
  CLI output are all written in English. Do not introduce other languages.

### Commits
- Make **atomic commits**: one logical change per commit. Commit each feature/fix/etc. as it
  is completed rather than batching unrelated changes together.
- Follow **[Conventional Commits](https://www.conventionalcommits.org/)**. Use the appropriate
  type prefix:
  - `feat:` — a new feature
  - `fix:` — a bug fix
  - `docs:` — documentation only
  - `refactor:` — code change that neither fixes a bug nor adds a feature
  - `chore:` — build, tooling, or housekeeping
  - `test:` — adding or fixing tests
  - `perf:`, `style:`, `ci:` as applicable
- Commit messages are in English, imperative mood (e.g. `fix: handle missing HOME variable`).

### Code quality
- Run `cargo fmt` before committing.
- Keep `cargo build` free of warnings.
- `Cargo.lock` is committed (this is a binary/application — pin dependencies for reproducible builds).
