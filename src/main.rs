//! ClaideInfo — scans the Claude Code history (~/.claude/projects/*/*.jsonl)
//! and reports, summing across all sessions, how long Claude worked and how many
//! tokens were spent. "Steam playtime" style view: overall total + per project.

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Accumulated statistics for a project.
#[derive(Default)]
struct Stats {
    /// Worked time, in milliseconds (sum of the `turn_duration` values reported by Claude).
    duration_ms: u64,
    /// Plain token total (input + output + cache_read + cache_creation).
    tokens: u64,
    /// Number of turns with a `turn_duration`.
    turns: u64,
    /// Number of sessions (.jsonl files).
    sessions: u64,
    /// Sessions without any `turn_duration` (old versions) — these produce no estimated time.
    sessions_without_time: u64,
    /// Human-readable project label (basename of `cwd`); filled in during the scan.
    label: String,
    /// Indicates `label` already came from `cwd` (preferred over the directory-name fallback).
    label_from_cwd: bool,
}

fn main() {
    let home = match env::var("HOME") {
        Ok(h) => h,
        Err(_) => {
            eprintln!("Error: HOME environment variable is not set.");
            std::process::exit(1);
        }
    };

    let projects_dir = Path::new(&home).join(".claude").join("projects");
    if !projects_dir.is_dir() {
        eprintln!(
            "Error: sessions directory not found: {}",
            projects_dir.display()
        );
        std::process::exit(1);
    }

    let mut by_project: HashMap<String, Stats> = HashMap::new();

    // GLOBAL deduplication by message.id: each API call is counted exactly once,
    // even when it shows up across several lines/files (main session and subagents repeat ids).
    let mut seen_ids: HashSet<String> = HashSet::new();

    let entries = match fs::read_dir(&projects_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error reading {}: {}", projects_dir.display(), e);
            std::process::exit(1);
        }
    };

    // Aggregation key = project directory name (stable). The human-readable label
    // (basename of cwd) is captured while scanning each session.
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().into_owned();
        let stats = by_project.entry(dir_name.clone()).or_default();
        if stats.label.is_empty() {
            stats.label = label_from_dir(&dir_name);
        }
        // `is_session=true` only for the .jsonl files directly under the project (real sessions).
        // Files in subfolders (e.g. <sessionId>/subagents/*.jsonl) count tokens/time,
        // but do not inflate the session count.
        for (file, is_session) in collect_jsonl(&path) {
            scan_session(&file, stats, &mut seen_ids, is_session);
        }
    }

    print_report(&by_project);
}

/// Recursively collects every `.jsonl` under a project directory.
/// Returns `(path, is_session)` pairs, where `is_session` is `true` only for the
/// files directly in the project directory (a real session = one top-level file).
fn collect_jsonl(project_dir: &Path) -> Vec<(PathBuf, bool)> {
    let mut out = Vec::new();
    let mut stack = vec![project_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().map(|x| x == "jsonl").unwrap_or(false) {
                let is_session = p.parent() == Some(project_dir);
                out.push((p, is_session));
            }
        }
    }
    out
}

/// Scans a `.jsonl` file line by line, accumulating time and tokens into the project's `stats`.
/// `seen_ids` is the GLOBAL set of `message.id` already counted (dedup across all files).
/// `is_session` indicates whether this file is a real session (top level) — only then are sessions counted.
fn scan_session(path: &Path, stats: &mut Stats, seen_ids: &mut HashSet<String>, is_session: bool) {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let reader = BufReader::new(file);

    let mut has_turn_duration = false;

    if is_session {
        stats.sessions += 1;
    }

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        // Malformed/partial lines are skipped silently.
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match v.get("type").and_then(Value::as_str) {
            Some("system") => {
                if v.get("subtype").and_then(Value::as_str) == Some("turn_duration") {
                    if let Some(ms) = v.get("durationMs").and_then(Value::as_u64) {
                        stats.duration_ms += ms;
                        stats.turns += 1;
                        has_turn_duration = true;
                    }
                }
            }
            Some("assistant") => {
                let msg = match v.get("message") {
                    Some(m) => m,
                    None => continue,
                };

                // Deduplication by message.id (safeguard against double counting).
                if let Some(id) = msg.get("id").and_then(Value::as_str) {
                    if !seen_ids.insert(id.to_string()) {
                        continue;
                    }
                }

                if let Some(usage) = msg.get("usage") {
                    stats.tokens += usage_total(usage);
                }

                // Human-readable project label, captured from the cwd of the first useful message.
                if !stats.label_from_cwd {
                    if let Some(cwd) = v.get("cwd").and_then(Value::as_str) {
                        if let Some(base) = basename(cwd) {
                            stats.label = base;
                            stats.label_from_cwd = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if is_session && !has_turn_duration {
        stats.sessions_without_time += 1;
    }
}

/// Sums the 4 token kinds of a `usage` object.
fn usage_total(usage: &Value) -> u64 {
    let g = |k: &str| usage.get(k).and_then(Value::as_u64).unwrap_or(0);
    g("input_tokens")
        + g("output_tokens")
        + g("cache_read_input_tokens")
        + g("cache_creation_input_tokens")
}

/// Basename of a path (last non-empty segment).
fn basename(path: &str) -> Option<String> {
    path.trim_end_matches('/')
        .rsplit('/')
        .find(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Fallback label derived from the directory name (`-home-antonio-dev-Foo` → `Foo`).
fn label_from_dir(dir_name: &str) -> String {
    dir_name
        .rsplit('-')
        .find(|s| !s.is_empty())
        .unwrap_or(dir_name)
        .to_string()
}

/// Formats milliseconds as "Xh Ymin".
fn fmt_dur(ms: u64) -> String {
    let total_min = ms / 60_000;
    let h = total_min / 60;
    let m = total_min % 60;
    format!("{}h {:02}min", h, m)
}

/// Formats an integer with a thousands separator (comma), en-US style.
fn fmt_int(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::new();
    let len = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

fn print_report(by_project: &HashMap<String, Stats>) {
    // Global totals.
    let total_ms: u64 = by_project.values().map(|s| s.duration_ms).sum();
    let total_tokens: u64 = by_project.values().map(|s| s.tokens).sum();
    let total_turns: u64 = by_project.values().map(|s| s.turns).sum();
    let total_sessions: u64 = by_project.values().map(|s| s.sessions).sum();
    let total_without_time: u64 = by_project.values().map(|s| s.sessions_without_time).sum();

    println!("ClaideInfo — Claude Code usage history\n");
    println!("OVERALL TOTAL");
    println!(
        "  Worked time : {}   ({} turns)",
        fmt_dur(total_ms),
        fmt_int(total_turns)
    );
    println!("  Tokens      : {}", fmt_int(total_tokens));
    if total_without_time > 0 {
        println!(
            "  Sessions    : {}  ({} without time data)",
            fmt_int(total_sessions),
            fmt_int(total_without_time)
        );
    } else {
        println!("  Sessions    : {}", fmt_int(total_sessions));
    }

    // Per-project ranking, sorted by time desc (tie-break by tokens desc).
    let mut rows: Vec<&Stats> = by_project.values().collect();
    rows.sort_by(|a, b| {
        b.duration_ms
            .cmp(&a.duration_ms)
            .then(b.tokens.cmp(&a.tokens))
    });

    // Pre-format to compute column widths.
    let prepared: Vec<(String, String, String, String)> = rows
        .iter()
        .map(|s| {
            (
                s.label.clone(),
                fmt_dur(s.duration_ms),
                fmt_int(s.tokens),
                fmt_int(s.sessions),
            )
        })
        .collect();

    let w_proj = prepared
        .iter()
        .map(|r| r.0.len())
        .chain(std::iter::once("Project".len()))
        .max()
        .unwrap_or(7);
    let w_time = prepared
        .iter()
        .map(|r| r.1.len())
        .chain(std::iter::once("Time".len()))
        .max()
        .unwrap_or(5);
    let w_tok = prepared
        .iter()
        .map(|r| r.2.len())
        .chain(std::iter::once("Tokens".len()))
        .max()
        .unwrap_or(6);

    println!("\nBY PROJECT (by time)");
    println!(
        "  {:<wp$}  {:<wt$}  {:<wk$}  {}",
        "Project",
        "Time",
        "Tokens",
        "Sessions",
        wp = w_proj,
        wt = w_time,
        wk = w_tok
    );
    for (proj, time, tok, sess) in &prepared {
        println!(
            "  {:<wp$}  {:<wt$}  {:<wk$}  {}",
            proj,
            time,
            tok,
            sess,
            wp = w_proj,
            wt = w_time,
            wk = w_tok
        );
    }
}
