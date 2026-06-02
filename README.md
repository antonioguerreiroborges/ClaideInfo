# ClaideInfo

A CLI tool that shows how much time Claude Code has worked on each of your projects and how many tokens were spent — a detailed usage history broken down per project.

```
ClaideInfo — Claude Code usage history

OVERALL TOTAL
  Worked time : 18h 42min   (284 turns)
  Tokens      : 391,847,203
  Sessions    : 74  (9 without time data)

BY PROJECT (by time)
  Project       Time      Tokens       Sessions
  my-saas-app   5h 53min  112,304,571  11
  rust-api       4h 12min  89,450,228   8
  mobile-app     3h 08min  67,391,004   19
  discord-bot    2h 14min  43,918,762   5
  portfolio      1h 22min  28,104,490   7
  cli-tools      0h 43min  19,837,201   4
  game-jam       0h 35min  14,209,512   3
  ...
```

---

## Requirements

- [Claude Code](https://claude.ai/code) installed with at least one session recorded (logs live in `~/.claude/projects/`)
- Git
- Rust + Cargo (see installation below)

---

## Install Rust

### Linux / macOS

The recommended way on any distribution or macOS version is **rustup**:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

> **Why rustup and not the system package manager?**  
> Most distributions ship outdated Rust versions. `rustup` always installs the latest stable release and is the officially supported method.

#### Arch Linux (alternative)
```bash
sudo pacman -S rust
```

#### Ubuntu / Debian (alternative — may be outdated)
```bash
sudo apt install cargo
```

---

### Windows

**Option 1 — Native Windows (recommended if you don't use WSL)**

1. Download and run `rustup-init.exe` from [rustup.rs](https://rustup.rs)
2. Follow the prompts — it will install the MSVC build tools automatically if needed
3. Open a new terminal and verify: `rustc --version`

**Option 2 — WSL2 (recommended if you already use WSL)**

Inside your WSL terminal, follow the Linux instructions above.  
Claude Code stores its logs in `~/.claude/projects/` inside WSL, so the tool will find them automatically.

---

## Clone and Build

```bash
git clone https://github.com/ANT-devolper/ClaideInfo.git
cd ClaideInfo
cargo build --release
```

The compiled binary will be at `target/release/claude-info`.

---

## Run

```bash
# Linux / macOS / WSL
./target/release/claude-info

# Windows (PowerShell)
.\target\release\claude-info.exe
```

Or build and run in one step:

```bash
cargo run --release
```

---

## How it works

- Reads all `.jsonl` session files under `~/.claude/projects/`
- Extracts worked time from `turn_duration` system events
- Extracts token counts from `assistant` messages (deduplicated by message ID so each API call is counted only once)
- Prints a total summary and a per-project ranking sorted by time spent
