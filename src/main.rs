mod ai;
mod app;
pub mod db;
mod d2l_plugin;
mod models;
mod theme;
mod ui;
mod vault;

use app::App;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::DefaultTerminal;
use std::io::stdout;
use std::process::Command;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--status") {
        let db = db::Database::open_default()?;
        println!("Database Cache: {}", db.path_display);
        println!("Supabase Configured: {}", db.has_supabase_configured());
        let pending = db.get_pending_sync_count().unwrap_or(0);
        println!("Pending Sync Items: {}", pending);
        let courses = db.load_courses().unwrap_or_default();
        let deadlines_count: usize = courses.iter().map(|c| c.items.len()).sum();
        println!("Cached Courses: {}, Deadlines: {}", courses.len(), deadlines_count);
        return Ok(());
    }

    if args.iter().any(|a| a == "--sync" || a == "--push") {
        let db = db::Database::open_default()?;
        println!("Connecting to Supabase and syncing...");
        match db.sync_full() {
            Ok(summary) => {
                println!("✅ {}", summary.message);
                return Ok(());
            }
            Err(e) => {
                eprintln!("❌ Sync failed: {}", e);
                std::process::exit(1);
            }
        }
    }

    let mut terminal = ratatui::init();
    terminal.clear()?;

    let custom_vault = std::env::args().nth(1).filter(|a| !a.starts_with('-')).map(std::path::PathBuf::from);
    let mut app = match App::new(custom_vault) {
        Ok(app) => app,
        Err(e) => {
            ratatui::restore();
            eprintln!("Failed to initialize dashboard: {}", e);
            std::process::exit(1);
        }
    };

    let tick_rate = Duration::from_millis(50);
    let res = run_loop(&mut terminal, &mut app, tick_rate);

    ratatui::restore();

    if let Err(err) = res {
        eprintln!("Application error: {:?}", err);
    }

    Ok(())
}

fn run_loop(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    tick_rate: Duration,
) -> anyhow::Result<()> {
    loop {
        // Advance tick: drain background worker results and advance spinner animation
        app.tick();

        // Handle suspension for external editor ($EDITOR / Neovim)
        if let Some(target_file) = app.suspend_for_editor.take() {
            disable_raw_mode()?;
            execute!(stdout(), LeaveAlternateScreen)?;

            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
            let status = Command::new(&editor)
                .arg(&target_file)
                .status();

            // Re-initialize terminal after editor exits
            enable_raw_mode()?;
            execute!(stdout(), EnterAlternateScreen)?;
            terminal.clear()?;

            match status {
                Ok(s) if s.success() => {
                    app.reload_vault();
                    app.set_status(format!("Edited {:?}", target_file.file_name().unwrap_or_default()));
                }
                _ => {
                    app.set_status("Editor exited or failed to run.");
                }
            }
        }

        // Handle suspension for D2L login (`d2l login`)
        if let Some(force) = app.suspend_for_d2l_login.take() {
            disable_raw_mode()?;
            execute!(stdout(), LeaveAlternateScreen)?;

            println!("\x1b[1;36m====================================================\x1b[0m");
            println!("\x1b[1;32m🔐 D2L Brightspace Authentication (`d2l login`)\x1b[0m");
            println!("\x1b[1;36m====================================================\x1b[0m\n");

            let mut cmd = Command::new("d2l");
            cmd.arg("login");
            if force {
                cmd.arg("--force");
            }
            let status = cmd.status();

            println!("\n\x1b[90mPress Enter to return to School-Dashboard...\x1b[0m");
            let mut buf = String::new();
            let _ = std::io::stdin().read_line(&mut buf);

            // Re-initialize terminal after login exits
            enable_raw_mode()?;
            execute!(stdout(), EnterAlternateScreen)?;
            terminal.clear()?;

            match status {
                Ok(s) if s.success() => {
                    app.d2l_enabled = d2l_plugin::D2lPlugin::is_available();
                    app.run_d2l_reconcile();
                    app.set_status("✅ D2L Authentication successful! Synced live deadlines & announcements.");
                }
                _ => {
                    app.set_status("❌ D2L Login process exited with errors or was cancelled.");
                }
            }
        }

        // Render frame
        terminal.draw(|f| ui::render(f, app))?;

        // Poll for inputs
        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }

        if app.should_quit {
            app.on_exit_sync();
            break;
        }
    }

    Ok(())
}
