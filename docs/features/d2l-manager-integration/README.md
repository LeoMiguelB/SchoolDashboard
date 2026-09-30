# 🔌 D2L Class Manager Integration (`d2l-plugin`)

The **D2L Class Manager Integration** bridges the high-speed local Terminal User Interface of `School-Dashboard` with live University of Guelph Brightspace/CourseLink data via the [`d2l-class-manager`](/home/lmb/Desktop/projects/d2l-class-manager) CLI tool.

---

## 🌟 Key Capabilities

1. **Zero-Dependency Optional Plugin Architecture**
   - Automatically detects if `d2l` is installed in `$PATH`.
   - If present, enables D2L features and displays `[󰒋 D2L Active]` in the dashboard header.
   - If absent, gracefully degrades with zero errors or disruption to local vault management.

2. **In-TUI Interactive Authentication (`L` / `:login`)**
   - Press `L` or type `:login` to trigger Brightspace authentication.
   - Suspends the TUI cleanly out of raw terminal mode so `d2l login` can run interactively, pop up Chrome for Duo 2FA / SSO if necessary, and cleanly restore the dashboard on completion.

3. **Non-Blocking Vault Cache Reconciliation (`S` / `:sync`)**
   - Treats your Obsidian `deadlines.json` files as the local source of truth and cache.
   - Runs on a **background worker thread** with channel communication (`std::sync::mpsc`). The TUI never blocks or freezes.
   - Displays a dynamic braille spinner (`[⠋ D2L Syncing...]`) in the header and status bar.
   - Queries live Brightspace dropbox folders, detecting due date shifts and auto-completing deliverables once work is submitted (`TotalFilesSubmitted > 0`).
   - Automatically creates `.bak` backups before modifying any local vault file.

4. **Non-Blocking Announcement-Aware AI Advisor (`a` / `:ai`)**
   - Ingests active course announcements alongside urgency horizon deliverables.
   - Generates tactical recommendations via **Gemini 3.8 Flash (Low)** asynchronously without UI freezes.
   - Shows `[⠋ Generating via Flash 3.8 Low...]` in the advisor title while synthesizing.
   - Powered by the local `agy` runner: `agy --model gemini-3.8-flash-low --print "<prompt>"`.

5. **Async Readings & Story Briefing for Tagged Courses (`R` / `:readings`)**
   - Courses declaring `"tags": ["readings"]` (e.g. `CTS*3020`) receive a `📖` badge in tabs and headers.
   - Instantly opens the briefing modal with an animated progress spinner (`⠋ Fetching CourseLink Content TOC & analyzing via Gemini 3.8 Flash (Low)...`).
   - Background thread queries Brightspace module descriptions, assigned stories, and external literature links.
   - Synthesizes upcoming lecture date, topic, assigned readings, Re:Vision story principles, and seminar preparation into the pop-up modal.

---

## ⌨️ Keybindings & Commands

| Key / Command | Scope | Description |
| :--- | :---: | :--- |
| `L` | Global | **D2L Login**: Suspends TUI to run `d2l login` (SSO/2FA browser or headless token refresh) |
| `:login`, `:auth` | Command | Initiate D2L authentication (`:login force` to force clean re-login) |
| `S` | Global | **D2L Sync & Reconcile**: Reconciles Brightspace dropboxes with local vault cache |
| `:sync`, `:reconcile` | Command | Run reconciliation report and apply updates |
| `R` | Global | **Readings Briefing**: Toggle next class reading & story prep briefing |
| `:readings` | Command | Open / close readings briefing pane |
| `a` | Global | **AI Strategic Advisor**: Re-query Gemini 3.8 Flash Low with announcements + deadlines |
| `:ai` | Command | Refresh AI recommendations |

---

## 🔄 Data Flow Architecture

```mermaid
flowchart TD
    subgraph Local Vault
        Obsidian["Obsidian Vault (F26)"]
        Deadlines["deadlines.json (Cache Layer)"]
        Tags["tags: ['readings']"]
    end

    subgraph School-Dashboard
        TUI["Ratatui Cockpit (schoodash)"]
        ReconcileEngine["Reconciliation Engine"]
        AiEngine["AiAdvisor (agy CLI)"]
    end

    subgraph D2L Tooling
        D2LBin["d2l CLI (~/.cargo/bin/d2l)"]
        BrightspaceAPI["Brightspace Valence API / CDP"]
    end

    subgraph AI Model
        Gemini["Gemini 3.8 Flash (Low)"]
    end

    Obsidian -->|Loads| Deadlines
    Deadlines -->|Reads| TUI
    TUI -->|'L' Key (Suspend)| D2LBin
    D2LBin -->|Browser SSO / 2FA| BrightspaceAPI
    TUI -->|'S' Key (Reconcile)| D2LBin
    D2LBin -->|Fetch Dropboxes & Announcements| BrightspaceAPI
    D2LBin -->|Reconcile Changes & Submissions| Deadlines
    TUI -->|'R' Key (TOC Content)| D2LBin
    D2LBin -->|Fetch Content TOC| BrightspaceAPI
    TUI -->|Deadlines + Announcements + Readings| AiEngine
    AiEngine -->|agy runner| Gemini
    Gemini -->|Strategic Advice & Story Briefing| TUI
```

---

## 🛠️ Obsidian Vault Schema Contract

To enable readings and direct LMS links, add `tags` and `links.courselink` to the course `deadlines.json`:

```json
{
  "course_code": "CTS*3020",
  "course_name": "Digital Storytelling",
  "instructor": "Prof. Shana MacDonald",
  "color": "magenta",
  "tags": ["readings"],
  "links": {
    "courselink": "https://courselink.uoguelph.ca/d2l/home/1077661"
  },
  "items": [ ... ]
}
```

- When `"tags": ["readings"]` is present, the course tab displays `[3] CTS*3020 📖`.
- Pressing `R` or `:readings` automatically selects this course and synthesizes the next class briefing.

---

## 🧪 Testing & Verification

1. **Unit Tests:**
   ```bash
   cargo test
   ```
   Verifies active pane navigation, tag filtering, discrepancy statistics, D2L plugin availability, sorting modes, and the `L` login keybinding.

2. **Manual Verification:**
   - Launch `schoodash`.
   - Verify `[󰒋 D2L Active]` is displayed in the top header.
   - Press `L`: observe clean suspension, terminal prompt, and return.
   - Press `S`: observe live dropbox reconciliation and auto-completion status.
   - Press `R`: inspect the readings & story prep popup window.
   - Press `a`: observe AI strategic advisor digest with live instructor announcements.
