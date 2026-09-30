# ⚡ Academic Cockpit: School-Dashboard (`schoodash`)

A blazing-fast, keyboard-driven Terminal User Interface (TUI) built in **Rust** with **Ratatui** for managing academic deadlines and coursework.

Designed to eliminate cognitive friction and monolithic markdown scanning by providing a horizon-filtered cockpit with modal Vim navigation, course policy inspection, and an AI Strategic Advisor.

---

## 📦 Installation & Setup

### Prerequisites
- A working Rust toolchain (`cargo` and `rustc` 1.80+).  
  If Rust is not yet installed on your machine, install it in seconds via [rustup](https://rustup.rs):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

---

### Effortless Installation (Choose Any Method)

#### Option A: One-Command Automated Installer (Recommended)
Clone the repository and run the included installer:

```bash
git clone https://github.com/your-username/School-Dashboard.git
cd School-Dashboard
./install.sh
```
*What this does:*
1. Compiles an optimized release binary.
2. Installs `schoodash` into `~/.local/bin/` (accessible user-wide without root).
3. Creates a `schooldash` alias.
4. Verifies your shell's `$PATH` configuration.

---

#### Option B: Standard Cargo Install
You can install directly using Cargo:

```bash
cd School-Dashboard
cargo install --path .
```
This builds and installs `schoodash` directly to `~/.cargo/bin/schoodash`.

---

#### Option C: Make
If you prefer `make`:

```bash
make install
```
To uninstall later:
```bash
make uninstall
```

---

## 🚀 Running the Dashboard

Once installed, simply type:

```bash
schoodash
```

*(Or use the alias `schooldash`).*

### Customizing Vault Location on Another Machine
By default, `schoodash` targets `/home/lmb/Documents/LeeOsVault/F26`. When cloning to a different machine or changing semesters, you can point it to any directory in two ways:

1. **Via CLI Argument:**
   ```bash
   schoodash ~/MyObsidianVault/F26
   ```

2. **Via Environment Variable (in `~/.bashrc` or `~/.zshrc`):**
   ```bash
   export SCHOOL_DASHBOARD_VAULT="$HOME/Documents/LeeOsVault/F26"
   ```

---

## ⌨️ Vim Motions & Keybindings

### Navigation Motions
| Key | Action |
| :--- | :--- |
| `j` / `k` or `↓` / `↑` | Move selection down / up (with auto-scrolling viewport) |
| `gg` / `G` | Jump to top / bottom of active list |
| `Ctrl-d` / `Ctrl-u` | Half-page jump down / up (5 items) |
| `Tab` / `Shift-Tab` | Cycle active panels (Focus $\leftrightarrow$ Radar $\leftrightarrow$ All Deadlines $\leftrightarrow$ AI Advisor) |
| `h` / `l` | Switch focus horizontally between lists and inspector |

### Action Verbs
| Key | Action |
| :--- | :--- |
| `<Space>` or `x` | **Toggle task status** (`todo` ⇄ `completed`). Automatically updates vault JSON! |
| `L` | **D2L Login**: Suspend TUI and authenticate with D2L Brightspace (`d2l login`), auto-restoring upon completion |
| `S` | **D2L Sync / Reconcile**: Reconcile live Brightspace dropboxes with vault cache and auto-complete submitted work |
| `R` | **Readings Briefing**: Toggle next class readings & story synthesis for tagged courses (`CTS*3020`) |
| `s` | **Cycle Deadline Sort**: `Closest → Farthest` (Default) ⇄ `Farthest → Closest` ⇄ `Highest Weight %` ⇄ `Course Code` |
| `c` | **Toggle Completed**: Show or hide completed deadlines in the All Deadlines window |
| `o` | **Open LMS Link**: Launches CourseLink / SOCS Moodle directly in default browser |
| `e` | **Edit in Neovim**: Suspends TUI and opens `deadlines.json` in `$EDITOR`, resuming on exit |
| `a` | **AI Advisor**: Re-query and refresh tactical academic recommendations with latest announcements |
| `t` | **Maximize Window**: Maximize / restore the All Deadlines window across the left panel |
| `r` | **Hot-Reload**: Reload all course JSON files from disk |
| `7` or `?` | **Help Page**: Open full-screen interactive keybindings cheat sheet (`1` or `Esc` to return) |
| `q` or `:q<CR>` | Quit cleanly |

### Tabs & Direct Course Filters
| Number Key | Tab Destination |
| :---: | :--- |
| `1` | `[1] All Classes` (Consolidated Aggregate View across all courses) |
| `2` | `[2] CIS*4300` (Human-Computer Interaction) |
| `3` | `[3] CTS*3020` (Digital Storytelling - 📖 Readings Tagged) |
| `4` | `[4] STAT*2050` (Statistics II) |
| `5` | `[5] CIS*3090` (Parallel Programming) |
| `6` | `[6] CIS*4020` (Data Science) |
| `7` or `?` | `[7] Help Page` (Keybinds & Vault reference) |

### Search & Command Mode
| Command | Action |
| :--- | :--- |
| `/` | Enter live fuzzy search mode across all tasks, tags, and notes (`Esc` to cancel) |
| `:login`, `:auth` | Launch D2L Brightspace authentication (`:login force` for clean re-login) |
| `:sync`, `:reconcile` | Reconcile Brightspace dropboxes and update local cache |
| `:readings` | Toggle Readings & Story Briefing for tagged courses |
| `:sort` | Cycle deadline sorting mode |
| `:completed` | Toggle visibility of completed tasks in All Deadlines |
| `:q` / `:quit` | Exit application |
| `:reload` | Reload vault data |
| `:ai` | Refresh AI summary (Gemini 3.8 Flash Low) |
| `:open` | Open selected item's LMS portal in browser |

---

## 🔌 D2L Class Manager Integration (Optional Plugin)

`schoodash` seamlessly integrates with [`d2l-class-manager`](/home/lmb/Desktop/projects/d2l-class-manager) as an optional, zero-overhead plugin to synchronize live University of Guelph Brightspace/CourseLink data.

### 1. Zero-Dependency Auto-Detection
- On startup, the dashboard checks if `d2l` is available in `$PATH`.
- **Installed:** Mounts the `[󰒋 D2L Active]` header badge and enables live synchronization.
- **Not Installed:** Gracefully runs in local-only mode with zero errors or degraded features.

### 2. Live Authentication & Terminal Suspension (`L` / `:login`)
- Press **`L`** (or type **`:login`** / **`:auth`**) to authenticate with Brightspace.
- **TUI Suspension:** The dashboard cleanly exits raw terminal mode and alternate screen, prints an auth banner, and runs `d2l login`.
- **Duo 2FA / SSO:** If a session needs renewal or 2FA, Chrome launches via Chrome DevTools Protocol (CDP) for one-click SSO. Once complete, session cookies and Bearer tokens are securely saved to `~/.config/d2l-manager/token.json`.
- **Smooth Return:** Prompts you to press `<Enter>` and automatically restores the TUI, immediately triggering an active data refresh.
- **Force Re-login:** Type **`:login force`** to force a clean browser login even if an active token exists.

### 3. Non-Blocking Vault Cache Reconciliation (`S` / `:sync`)
- Press **`S`** (or type **`:sync`** / **`:reconcile`**) to reconcile live LMS dropboxes with your local Obsidian vault in the background.
- **Asynchronous Execution:** Runs on a background worker thread—the dashboard **never freezes** and remains 100% interactive while syncing.
- **Animated Indicator:** Displays `[⠋ D2L Syncing...]` with a spinning braille animation in the header and status bar.
- **Local Cache Contract:** Local `deadlines.json` files serve as your local source of truth.
- **Auto-Completion & Drift Detection:** Automatically detects due date shifts and marks deliverables as `completed` once files are submitted (`TotalFilesSubmitted > 0`).
- **Safety Backups:** Every file write automatically creates a timestamped `.bak` backup file.

### 4. Non-Blocking Announcement-Aware AI Advisor (`a` / `:ai`)
- Combines pending deadline radar with live instructor announcements (e.g. test tips, guest lecturer notices, deadline extensions).
- **Asynchronous Generation:** Pressing **`a`** triggers background synthesis via **Gemini 3.8 Flash (Low)** without locking the TUI:
  ```bash
  agy --model gemini-3.8-flash-low --print "<prompt>"
  ```
- **Animated Indicator:** Shows `[⠋ Generating via Flash 3.8 Low...]` in the AI box title while thinking.
- **Persistent Cache:** Cached locally (`ai_summary_cache.json`) for instant sub-millisecond retrieval with graceful deterministic fallback.

### 5. Async Readings & Story Briefings for Tagged Courses (`R` / `:readings`)
- Courses with `"tags": ["readings"]` (e.g., `CTS*3020`) receive a `📖` badge in the tab bar and header.
- Press **`R`** (or type **`:readings`**) to open a dedicated briefing modal.
- **Instant Display:** Instantly opens the modal with an animated progress spinner (`⠋ Fetching CourseLink Content TOC & analyzing via Gemini 3.8 Flash (Low)...`) while the background thread queries Brightspace module descriptions and assigned reading URLs.
- **Deep Synthesis:** Prompts Gemini 3.8 Flash (Low) to summarize next class readings, author arguments, external story links (e.g. Re:Vision digital storymaking), and seminar discussion preparation.

*For full technical specifications, see [`docs/features/d2l-manager-integration/README.md`](docs/features/d2l-manager-integration/README.md).*

---

## 📂 Vault Directory Structure & Schema

The cockpit expects a modular folder per course containing a `deadlines.json` file:

```
$SCHOOL_DASHBOARD_VAULT/
├── CIS4300/
│   ├── deadlines.json
│   └── Deadlines.md
├── CTS3020/
│   ├── deadlines.json
│   └── Deadlines.md
├── STAT2050/
│   ├── deadlines.json
│   └── Deadlines.md
├── CIS3090/
│   ├── deadlines.json
│   └── Deadlines.md
└── CIS4020/
    ├── deadlines.json
    └── Deadlines.md
```

### Example `deadlines.json` (with Readings Tag & LMS Link)
```json
{
  "course_code": "CTS*3020",
  "course_name": "Digital Storytelling",
  "instructor": "Prof. Shana MacDonald",
  "color": "magenta",
  "tags": [
    "readings"
  ],
  "links": {
    "courselink": "https://courselink.uoguelph.ca/d2l/home/1077661"
  },
  "policies": {
    "grace_days": "1 automatic 48-hour grace token per semester",
    "late_penalty": "5% per day deduction after grace period",
    "reading_prep": "Must prepare 1 question per assigned reading for Tuesday seminars"
  },
  "items": [
    {
      "id": "cts3020-critique",
      "title": "Critical Reflection & Story Pitch",
      "type": "assignment",
      "due_date": "2026-10-15T23:59:00-04:00",
      "weight": 20.0,
      "status": "todo",
      "submission_platform": "CourseLink Dropbox",
      "lead_time_days": 10,
      "notes": "Integrate principles from Rice & Mündel (2018) reading and Re:Vision archive."
    }
  ]
}
```

---

## 🏗️ Architecture & Development

- **Framework:** [Ratatui 0.30](https://github.com/ratatui/ratatui) + [Crossterm 0.29](https://github.com/crossterm-rs/crossterm)
- **Serialization:** Serde JSON + Chrono with timezone awareness
- **Style Theme:** High-contrast Catppuccin Mocha / Tokyo Night dark palette
- **AI Engine:** Gemini 3.8 Flash (Low) via `agy` CLI runner
- **Plugin Companion:** `d2l-class-manager` (Rust / CDP / Valence API)

### Running Tests
```bash
cargo test
```
