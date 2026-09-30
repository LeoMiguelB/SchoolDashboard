use crate::ai::{AiAdvisor, AiSummary, ReadingsSummary};
use crate::d2l_plugin::{AnnouncementSummary, D2lPlugin, ReconciliationReport};
use crate::db::{Database, SyncSummary};
use crate::models::{Course, DeadlineItem, DeadlineSortOrder, ItemStatus};
use crate::vault::DEFAULT_VAULT_PATH;
use chrono::{DateTime, Local};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivePane {
    ImmediateFocus,
    OnTheRadar,
    AllDeadlines,
    AiAdvisor,
    Readings,
}

impl ActivePane {
    pub fn next(&self, has_readings: bool) -> Self {
        match self {
            ActivePane::ImmediateFocus => ActivePane::OnTheRadar,
            ActivePane::OnTheRadar => ActivePane::AllDeadlines,
            ActivePane::AllDeadlines => ActivePane::AiAdvisor,
            ActivePane::AiAdvisor => {
                if has_readings {
                    ActivePane::Readings
                } else {
                    ActivePane::ImmediateFocus
                }
            }
            ActivePane::Readings => ActivePane::ImmediateFocus,
        }
    }

    pub fn prev(&self, has_readings: bool) -> Self {
        match self {
            ActivePane::ImmediateFocus => {
                if has_readings {
                    ActivePane::Readings
                } else {
                    ActivePane::AiAdvisor
                }
            }
            ActivePane::OnTheRadar => ActivePane::ImmediateFocus,
            ActivePane::AllDeadlines => ActivePane::OnTheRadar,
            ActivePane::AiAdvisor => ActivePane::AllDeadlines,
            ActivePane::Readings => ActivePane::AiAdvisor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Search,
    Command,
    Help,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PendingStatusUpdate {
    pub course_code: String,
    pub item_id: String,
    pub target_status: ItemStatus,
    pub title: String,
}

#[derive(Debug)]
pub enum BackgroundResult {
    ReconcileFinished(Result<Vec<ReconciliationReport>, String>),
    ReadingsFinished {
        course_code: String,
        result: Result<ReadingsSummary, String>,
    },
    AiSummaryFinished(Result<AiSummary, String>),
    ItemStatusSaved {
        course_code: String,
        item_id: String,
        new_status: ItemStatus,
        result: Result<(), String>,
    },
    SyncFinished(Result<SyncSummary, String>),
}

pub struct App {
    pub vault_path: PathBuf,
    pub db: Database,
    pub is_syncing_supabase: bool,
    pub courses: Vec<Course>,
    pub active_pane: ActivePane,
    pub focus_index: usize,
    pub radar_index: usize,
    pub all_index: usize,
    pub selected_course_filter: Option<String>,
    pub mode: AppMode,
    pub search_query: String,
    pub command_query: String,
    pub status_message: Option<(String, Instant)>,
    pub ai_summary: AiSummary,
    pub should_quit: bool,
    pub suspend_for_editor: Option<PathBuf>,
    pub suspend_for_d2l_login: Option<bool>,
    pub g_pressed: bool,
    pub view_all_tab: bool,
    pub sort_order: DeadlineSortOrder,
    pub show_completed: bool,

    // D2L Plugin additions
    pub d2l_enabled: bool,
    pub reconcile_reports: Option<Vec<ReconciliationReport>>,
    pub readings_summary: Option<ReadingsSummary>,
    pub show_readings_pane: bool,

    // Async background task channels & states
    pub bg_sender: std::sync::mpsc::Sender<BackgroundResult>,
    pub bg_receiver: std::sync::mpsc::Receiver<BackgroundResult>,
    pub is_reconciling: bool,
    pub is_loading_readings: bool,
    pub is_generating_ai: bool,
    pub spinner_frame: usize,
    pub pending_status_updates: HashMap<String, PendingStatusUpdate>,
}

impl App {
    pub fn new(vault_path_override: Option<PathBuf>) -> anyhow::Result<Self> {
        let vault_path = vault_path_override
            .or_else(|| std::env::var("SCHOOL_DASHBOARD_VAULT").ok().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from(DEFAULT_VAULT_PATH));

        let db = Database::open_default()?;
        let _ = db.bootstrap_from_vault_deprecated_if_empty(&vault_path);

        let mut initial_sync_msg = None;
        if db.has_supabase_configured() {
            match db.sync_full() {
                Ok(summary) => {
                    initial_sync_msg = Some(summary.message);
                }
                Err(e) => {
                    initial_sync_msg = Some(format!("Offline mode (Supabase sync failed: {})", e));
                }
            }
        }

        let courses = db.load_courses().unwrap_or_default();
        let now = Local::now();

        // 1. Detect if D2L plugin is installed in PATH
        let d2l_enabled = D2lPlugin::is_available();

        let all_items: Vec<_> = courses.iter().flat_map(|c| c.items.clone()).collect();
        let ai_summary = AiAdvisor::load_cached()
            .unwrap_or_else(|| AiAdvisor::generate_heuristic_summary(&all_items, &[], now));

        // Load cached readings for courses with readings tag
        let readings_course = courses.iter().find(|c| c.has_readings()).map(|c| c.course_code.clone());
        let readings_summary = readings_course.as_deref().and_then(AiAdvisor::load_cached_readings);

        let welcome_msg = if let Some(msg) = initial_sync_msg {
            msg
        } else if d2l_enabled {
            "⚡ Academic Cockpit [SQLite Cache + D2L Active]. Press 'S' to sync, 'R' for readings, '?' for help.".to_string()
        } else {
            "⚡ Academic Cockpit [SQLite Cache]. Press '?' for Vim motions.".to_string()
        };

        let (bg_sender, bg_receiver) = std::sync::mpsc::channel();

        let app = Self {
            vault_path,
            db,
            is_syncing_supabase: false,
            courses,
            active_pane: ActivePane::ImmediateFocus,
            focus_index: 0,
            radar_index: 0,
            all_index: 0,
            selected_course_filter: None,
            mode: AppMode::Normal,
            search_query: String::new(),
            command_query: String::new(),
            status_message: Some((welcome_msg.to_string(), Instant::now())),
            ai_summary,
            should_quit: false,
            suspend_for_editor: None,
            suspend_for_d2l_login: None,
            g_pressed: false,
            view_all_tab: false,
            sort_order: DeadlineSortOrder::ClosestFirst,
            show_completed: true,
            d2l_enabled,
            reconcile_reports: None,
            readings_summary,
            show_readings_pane: false,
            bg_sender,
            bg_receiver,
            is_reconciling: false,
            is_loading_readings: false,
            is_generating_ai: false,
            spinner_frame: 0,
            pending_status_updates: HashMap::new(),
        };

        Ok(app)
    }

    pub fn is_item_pending(&self, course_code: &str, item_id: &str) -> bool {
        let key = format!("{}:{}", course_code, item_id);
        self.pending_status_updates.contains_key(&key)
    }

    #[allow(dead_code)]
    pub fn pending_update_for(&self, course_code: &str, item_id: &str) -> Option<&PendingStatusUpdate> {
        let key = format!("{}:{}", course_code, item_id);
        self.pending_status_updates.get(&key)
    }

    pub fn is_any_status_pending(&self) -> bool {
        !self.pending_status_updates.is_empty()
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), Instant::now()));
    }

    pub fn selected_course(&self) -> Option<&Course> {
        if let Some(ref cf) = self.selected_course_filter {
            self.courses.iter().find(|c| c.course_code == *cf)
        } else {
            None
        }
    }

    pub fn current_has_readings(&self) -> bool {
        if let Some(course) = self.selected_course() {
            course.has_readings()
        } else {
            self.courses.iter().any(|c| c.has_readings())
        }
    }

    pub fn reload_vault(&mut self) {
        match self.db.load_courses() {
            Ok(courses) => {
                self.courses = courses;
                let now = Local::now();
                let all_items = self.all_deadline_items();
                let announcements: Vec<AnnouncementSummary> = self
                    .reconcile_reports
                    .as_ref()
                    .map(|reps| reps.iter().flat_map(|r| r.announcements_for_ai.clone()).collect())
                    .unwrap_or_default();

                self.ai_summary = AiAdvisor::generate_heuristic_summary(&all_items, &announcements, now);
                self.clamp_indices(now);
                self.set_status("✔ Database reloaded successfully.");
                if !self.is_generating_ai {
                    self.refresh_ai_summary_silent(now);
                }
            }
            Err(e) => {
                self.set_status(format!("✖ Error loading database: {}", e));
            }
        }
    }

    pub fn reload_database(&mut self) {
        self.reload_vault();
    }

    pub fn sync_with_supabase(&mut self) {
        if self.is_syncing_supabase {
            self.set_status("⏳ Sync already in progress...");
            return;
        }
        self.is_syncing_supabase = true;
        self.set_status("🔄 Syncing with Supabase connection in background...");

        let sender = self.bg_sender.clone();
        let db_path = PathBuf::from(&self.db.path_display);
        std::thread::spawn(move || {
            let res = Database::open(&db_path)
                .and_then(|db| db.sync_full())
                .map_err(|e| e.to_string());
            let _ = sender.send(BackgroundResult::SyncFinished(res));
        });
    }

    pub fn on_exit_sync(&mut self) {
        if self.db.has_supabase_configured() {
            if let Ok(count) = self.db.get_pending_sync_count() {
                if count > 0 {
                    let _ = self.db.sync_full();
                }
            }
        }
    }

    pub fn tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);

        while let Ok(msg) = self.bg_receiver.try_recv() {
            match msg {
                BackgroundResult::SyncFinished(res) => {
                    self.is_syncing_supabase = false;
                    match res {
                        Ok(summary) => {
                            self.reload_database();
                            self.set_status(summary.message);
                        }
                        Err(e) => {
                            self.set_status(format!("❌ Supabase sync failed: {}", e));
                        }
                    }
                }
                BackgroundResult::ReconcileFinished(res) => {
                    self.is_reconciling = false;
                    match res {
                        Ok(reports) => {
                            let updated_items = self.db.apply_reconciliation(&reports).unwrap_or(0);
                            let (shifts, autocompleted, missing) = D2lPlugin::discrepancy_stats(&reports);
                            let total_pulled: usize = reports.iter().map(|r| r.pulled_content.len()).sum();
                            if updated_items > 0 || shifts > 0 || autocompleted > 0 || total_pulled > 0 {
                                self.reload_database();
                            }
                            self.reconcile_reports = Some(reports);

                            let mut status_parts = Vec::new();
                            if shifts > 0 {
                                status_parts.push(format!("{} due date{}", shifts, if shifts == 1 { "" } else { "s" }));
                            }
                            if autocompleted > 0 {
                                status_parts.push(format!("{} completed", autocompleted));
                            }
                            if missing > 0 {
                                status_parts.push(format!("{} missing in vault", missing));
                            }
                            if total_pulled > 0 {
                                status_parts.push(format!("{} new file{} pulled", total_pulled, if total_pulled == 1 { "" } else { "s" }));
                            }

                            if status_parts.is_empty() {
                                self.set_status("✅ D2L Sync: All courses & materials in sync! Announcements updated.");
                            } else {
                                self.set_status(format!("✅ D2L Reconciled: {}", status_parts.join(", ")));
                            }
                        }
                        Err(e) => {
                            self.set_status(format!("❌ D2L Reconcile failed: {}", e));
                        }
                    }
                }
                BackgroundResult::ReadingsFinished { course_code, result } => {
                    self.is_loading_readings = false;
                    match result {
                        Ok(summary) => {
                            self.readings_summary = Some(summary);
                            self.set_status(format!("📖 Readings & Story Prep ready for {}.", course_code));
                        }
                        Err(e) => {
                            self.set_status(format!("❌ Failed to generate readings for {}: {}", course_code, e));
                        }
                    }
                }
                BackgroundResult::AiSummaryFinished(result) => {
                    self.is_generating_ai = false;
                    match result {
                        Ok(summary) => {
                            self.ai_summary = summary;
                            self.set_status("🤖 AI Strategic Advisor refreshed with latest announcements.");
                        }
                        Err(e) => {
                            self.set_status(format!("❌ AI generation failed: {}", e));
                        }
                    }
                }
                BackgroundResult::ItemStatusSaved {
                    course_code,
                    item_id,
                    new_status,
                    result,
                } => {
                    let key = format!("{}:{}", course_code, item_id);
                    let pending = self.pending_status_updates.remove(&key);
                    let item_title = pending.map(|p| p.title).unwrap_or_else(|| item_id.clone());

                    match result {
                        Ok(()) => {
                            for course in &mut self.courses {
                                if course.course_code == course_code {
                                    for item in &mut course.items {
                                        if item.id == item_id {
                                            item.status = new_status;
                                            break;
                                        }
                                    }
                                }
                            }

                            let now = Local::now();
                            self.clamp_indices(now);

                            let all_items = self.all_deadline_items();
                            let announcements: Vec<AnnouncementSummary> = self
                                .reconcile_reports
                                .as_ref()
                                .map(|reps| reps.iter().flat_map(|r| r.announcements_for_ai.clone()).collect())
                                .unwrap_or_default();
                            self.ai_summary = AiAdvisor::generate_heuristic_summary(&all_items, &announcements, now);

                            self.set_status(format!("✔ Marked '{}' as {:?}.", item_title, new_status));

                            if !self.is_generating_ai {
                                self.refresh_ai_summary_silent(now);
                            }
                        }
                        Err(e) => {
                            self.set_status(format!("❌ Failed to save status for '{}': {}", item_title, e));
                        }
                    }
                }
            }
        }
    }

    pub fn run_d2l_reconcile(&mut self) {
        if !self.d2l_enabled {
            self.set_status("D2L plugin not available (d2l CLI not found in PATH).");
            return;
        }
        if self.is_reconciling {
            self.set_status("⏳ D2L sync already in progress in background...");
            return;
        }

        self.is_reconciling = true;
        self.set_status("🔄 Syncing live data from D2L CourseLink in background...");

        let sender = self.bg_sender.clone();
        let vault_path = self.vault_path.clone();
        std::thread::spawn(move || {
            let res = D2lPlugin::run_reconcile(true, Some(&vault_path))
                .map_err(|e| e.to_string());
            let _ = sender.send(BackgroundResult::ReconcileFinished(res));
        });
    }

    pub fn initiate_d2l_login(&mut self, force: bool) {
        if !D2lPlugin::is_available() {
            self.set_status("❌ 'd2l' CLI binary not found in PATH. Install d2l-class-manager to authenticate.");
            return;
        }
        self.suspend_for_d2l_login = Some(force);
        self.set_status("🔐 Launching D2L Brightspace authentication (`d2l login`)...");
    }

    pub fn refresh_ai_summary(&mut self, now: DateTime<Local>) {
        self.refresh_ai_summary_opts(now, true);
    }

    pub fn refresh_ai_summary_silent(&mut self, now: DateTime<Local>) {
        self.refresh_ai_summary_opts(now, false);
    }

    fn refresh_ai_summary_opts(&mut self, now: DateTime<Local>, show_status: bool) {
        if self.is_generating_ai {
            if show_status {
                self.set_status("⏳ AI advisor generation already in progress...");
            }
            return;
        }
        self.is_generating_ai = true;
        if show_status {
            self.set_status("🤖 Querying Gemini 3.8 Flash (Low) with deadlines & announcements in background...");
        }

        let sender = self.bg_sender.clone();
        let all_items = self.all_deadline_items();
        let announcements: Vec<AnnouncementSummary> = self
            .reconcile_reports
            .as_ref()
            .map(|reps| reps.iter().flat_map(|r| r.announcements_for_ai.clone()).collect())
            .unwrap_or_default();

        std::thread::spawn(move || {
            let summary = AiAdvisor::generate_summary(&all_items, &announcements, now);
            let _ = sender.send(BackgroundResult::AiSummaryFinished(Ok(summary)));
        });
    }

    pub fn toggle_or_refresh_readings(&mut self, now: DateTime<Local>) {
        let course = self.selected_course()
            .filter(|c| c.has_readings())
            .or_else(|| self.courses.iter().find(|c| c.has_readings()));

        let target_code = match course {
            Some(c) => c.course_code.clone(),
            None => {
                self.set_status("No course currently has the 'readings' tag.");
                return;
            }
        };

        self.show_readings_pane = !self.show_readings_pane;
        if self.show_readings_pane {
            self.active_pane = ActivePane::Readings;
            let needs_load = self.readings_summary.as_ref().map(|r| r.course_code != target_code).unwrap_or(true);
            if needs_load && !self.is_loading_readings {
                self.load_readings_async(&target_code, now);
            } else if self.is_loading_readings {
                self.set_status(format!("📖 Loading Readings & Story Prep for {} in background...", target_code));
            } else {
                self.set_status(format!("📖 Viewing Readings & Story Prep for {}.", target_code));
            }
        } else {
            self.active_pane = ActivePane::ImmediateFocus;
            self.set_status("Closed Readings view.");
        }
    }

    pub fn load_readings_async(&mut self, target_code: &str, now: DateTime<Local>) {
        if self.is_loading_readings {
            return;
        }
        self.is_loading_readings = true;
        self.set_status(format!("📖 Loading readings for {} via Gemini 3.8 Flash (Low)...", target_code));

        let sender = self.bg_sender.clone();
        let code = target_code.to_string();
        let d2l_enabled = self.d2l_enabled;

        std::thread::spawn(move || {
            let context = if d2l_enabled {
                D2lPlugin::fetch_course_content(&code)
                    .map(|val| D2lPlugin::extract_readings_context(&code, &val))
                    .unwrap_or_else(|_| format!("Course {} requires readings for each Tuesday class.", code))
            } else {
                format!("Course {} requires readings for each Tuesday class.", code)
            };

            let summary_opt = AiAdvisor::generate_readings_summary(&code, &context, now);
            let result = summary_opt.ok_or_else(|| format!("Could not generate readings summary for {}", code));
            let _ = sender.send(BackgroundResult::ReadingsFinished {
                course_code: code,
                result,
            });
        });
    }

    pub fn all_deadline_items(&self) -> Vec<DeadlineItem> {
        let mut items = Vec::new();
        for course in &self.courses {
            if let Some(ref filter) = self.selected_course_filter {
                if !course.course_code.eq_ignore_ascii_case(filter) {
                    continue;
                }
            }
            items.extend(course.items.clone());
        }

        // Apply search query filter if active
        if !self.search_query.trim().is_empty() {
            let q = self.search_query.to_lowercase();
            items.retain(|item| {
                item.title.to_lowercase().contains(&q)
                    || item.course_code.to_lowercase().contains(&q)
                    || item.notes.as_deref().unwrap_or("").to_lowercase().contains(&q)
            });
        }

        items
    }

    pub fn immediate_focus_items(&self, now: DateTime<Local>) -> Vec<DeadlineItem> {
        let mut items: Vec<_> = self
            .all_deadline_items()
            .into_iter()
            .filter(|i| i.is_immediate_focus(now))
            .collect();

        items.sort_by(|a, b| {
            a.priority_score(now)
                .partial_cmp(&b.priority_score(now))
                .unwrap_or(std::cmp::Ordering::Equal)
                .reverse()
        });

        items
    }

    pub fn radar_items(&self, now: DateTime<Local>) -> Vec<DeadlineItem> {
        let mut items: Vec<_> = self
            .all_deadline_items()
            .into_iter()
            .filter(|i| i.is_on_radar(now))
            .collect();

        items.sort_by(|a, b| a.due_date.cmp(&b.due_date));
        items
    }

    #[allow(dead_code)]
    pub fn full_roadmap_items(&self) -> Vec<DeadlineItem> {
        self.all_deadlines_sorted()
    }

    pub fn all_deadlines_sorted(&self) -> Vec<DeadlineItem> {
        let mut items = self.all_deadline_items();

        if !self.show_completed {
            items.retain(|i| !i.status.is_done());
        }

        match self.sort_order {
            DeadlineSortOrder::ClosestFirst => {
                items.sort_by(|a, b| a.due_date.cmp(&b.due_date));
            }
            DeadlineSortOrder::FarthestFirst => {
                items.sort_by(|a, b| b.due_date.cmp(&a.due_date));
            }
            DeadlineSortOrder::WeightHighest => {
                items.sort_by(|a, b| {
                    b.weight
                        .partial_cmp(&a.weight)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.due_date.cmp(&b.due_date))
                });
            }
            DeadlineSortOrder::CourseCode => {
                items.sort_by(|a, b| {
                    a.course_code
                        .cmp(&b.course_code)
                        .then_with(|| a.due_date.cmp(&b.due_date))
                });
            }
        }

        items
    }

    pub fn clamp_indices(&mut self, now: DateTime<Local>) {
        let focus_len = self.immediate_focus_items(now).len();
        if focus_len == 0 {
            self.focus_index = 0;
        } else if self.focus_index >= focus_len {
            self.focus_index = focus_len - 1;
        }

        let radar_len = self.radar_items(now).len();
        if radar_len == 0 {
            self.radar_index = 0;
        } else if self.radar_index >= radar_len {
            self.radar_index = radar_len - 1;
        }

        let all_len = self.all_deadlines_sorted().len();
        if all_len == 0 {
            self.all_index = 0;
        } else if self.all_index >= all_len {
            self.all_index = all_len - 1;
        }
    }

    pub fn selected_item(&self, now: DateTime<Local>) -> Option<DeadlineItem> {
        match self.active_pane {
            ActivePane::ImmediateFocus => {
                let items = self.immediate_focus_items(now);
                items.get(self.focus_index).cloned()
            }
            ActivePane::OnTheRadar => {
                let items = self.radar_items(now);
                items.get(self.radar_index).cloned()
            }
            ActivePane::AllDeadlines => {
                let items = self.all_deadlines_sorted();
                items.get(self.all_index).cloned()
            }
            ActivePane::AiAdvisor | ActivePane::Readings => {
                let items = self.immediate_focus_items(now);
                items.first().cloned()
            }
        }
    }

    pub fn toggle_selected_status(&mut self, now: DateTime<Local>) {
        if let Some(item) = self.selected_item(now) {
            let key = format!("{}:{}", item.course_code, item.id);
            if self.pending_status_updates.contains_key(&key) {
                self.set_status(format!("⏳ '{}' is already being updated...", item.title));
                return;
            }

            let next_status = item.status.toggle();
            let course_code = item.course_code.clone();
            let item_id = item.id.clone();
            let title = item.title.clone();
            let db_path = PathBuf::from(&self.db.path_display);
            let sender = self.bg_sender.clone();

            self.pending_status_updates.insert(
                key,
                PendingStatusUpdate {
                    course_code: course_code.clone(),
                    item_id: item_id.clone(),
                    target_status: next_status,
                    title: title.clone(),
                },
            );

            self.set_status(format!(
                "⏳ Saving '{}' as {:?}...",
                title, next_status
            ));

            std::thread::spawn(move || {
                let start = std::time::Instant::now();
                let res = Database::open(&db_path)
                    .and_then(|db| db.save_item_status(&course_code, &item_id, next_status))
                    .map_err(|e| e.to_string());

                // Keep loading indicator visible for smooth perceptible confirmation (~350ms)
                let min_display = std::time::Duration::from_millis(350);
                if let Some(remaining) = min_display.checked_sub(start.elapsed()) {
                    std::thread::sleep(remaining);
                }

                let _ = sender.send(BackgroundResult::ItemStatusSaved {
                    course_code,
                    item_id,
                    new_status: next_status,
                    result: res,
                });
            });
        }
    }

    pub fn open_selected_link(&mut self, now: DateTime<Local>) {
        if let Some(item) = self.selected_item(now) {
            let course = self.courses.iter().find(|c| c.course_code == item.course_code);
            let mut opened = false;

            if let Some(course) = course {
                if let Some(ref links) = course.links {
                    for (_key, url) in links {
                        if url.starts_with("http") {
                            let _ = open::that(url);
                            self.set_status(format!("Opening link: {}", url));
                            opened = true;
                            break;
                        }
                    }
                }
            }

            if !opened {
                let fallback = "https://courselink.uoguelph.ca";
                let _ = open::that(fallback);
                self.set_status(format!("Opening {}", fallback));
            }
        }
    }

    pub fn open_selected_in_editor(&mut self, now: DateTime<Local>) {
        if let Some(item) = self.selected_item(now) {
            let course_dir = self.vault_path.join(item.course_code.replace('*', ""));
            let md_file = course_dir.join("Deadlines.md");
            let dep_file = course_dir.join("deadlines.deprecated.json");
            let dep_file2 = course_dir.join("deadline.deprecated.json");
            if md_file.exists() {
                self.suspend_for_editor = Some(md_file);
            } else if dep_file.exists() {
                self.suspend_for_editor = Some(dep_file);
            } else if dep_file2.exists() {
                self.suspend_for_editor = Some(dep_file2);
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.mode {
            AppMode::Normal => self.handle_normal_mode(key),
            AppMode::Search => self.handle_search_mode(key),
            AppMode::Command => self.handle_command_mode(key),
            AppMode::Help => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                        self.mode = AppMode::Normal;
                    }
                    KeyCode::Char('1') => {
                        self.selected_course_filter = None;
                        self.mode = AppMode::Normal;
                        self.set_status("Filter: Showing All Classes (Aggregate View)");
                    }
                    KeyCode::Char('2') => {
                        self.mode = AppMode::Normal;
                        self.filter_course_index(0);
                    }
                    KeyCode::Char('3') => {
                        self.mode = AppMode::Normal;
                        self.filter_course_index(1);
                    }
                    KeyCode::Char('4') => {
                        self.mode = AppMode::Normal;
                        self.filter_course_index(2);
                    }
                    KeyCode::Char('5') => {
                        self.mode = AppMode::Normal;
                        self.filter_course_index(3);
                    }
                    KeyCode::Char('6') => {
                        self.mode = AppMode::Normal;
                        self.filter_course_index(4);
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_normal_mode(&mut self, key: KeyEvent) {
        let now = Local::now();

        // Check for 'g' sequence (gg = jump to top)
        if self.g_pressed {
            self.g_pressed = false;
            if key.code == KeyCode::Char('g') {
                self.jump_to_top();
                return;
            }
        }

        let has_readings = self.show_readings_pane || self.current_has_readings();

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.mode = AppMode::Help,
            KeyCode::Char('/') => {
                self.mode = AppMode::Search;
                self.search_query.clear();
            }
            KeyCode::Char(':') => {
                self.mode = AppMode::Command;
                self.command_query.clear();
            }

            // Vim Motions
            KeyCode::Char('j') | KeyCode::Down => self.move_cursor_down(now),
            KeyCode::Char('k') | KeyCode::Up => self.move_cursor_up(),
            KeyCode::Char('G') => self.jump_to_bottom(now),
            KeyCode::Char('g') => {
                self.g_pressed = true;
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.page_down(now);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.page_up();
            }

            // Panes & Navigation
            KeyCode::Tab => {
                self.active_pane = self.active_pane.next(has_readings);
            }
            KeyCode::BackTab => {
                self.active_pane = self.active_pane.prev(has_readings);
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.active_pane = self.active_pane.prev(has_readings);
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.active_pane = self.active_pane.next(has_readings);
            }

            // Quick Course Switching (1 = All Classes, 2-6 = individual courses)
            KeyCode::Char('1') => {
                self.selected_course_filter = None;
                self.clamp_indices(now);
                self.set_status("Filter: Showing All Classes (Aggregate View)");
            }
            KeyCode::Char('2') => self.filter_course_index(0),
            KeyCode::Char('3') => self.filter_course_index(1),
            KeyCode::Char('4') => self.filter_course_index(2),
            KeyCode::Char('5') => self.filter_course_index(3),
            KeyCode::Char('6') => self.filter_course_index(4),
            KeyCode::Char('7') => {
                self.mode = AppMode::Help;
                self.set_status("Help Page: View all keybinds and features. Press '1' or 'Esc' to return.");
            }
            KeyCode::Char('0') => {
                self.selected_course_filter = None;
                self.clamp_indices(now);
                self.set_status("Filter: Showing All Classes (Aggregate View)");
            }

            // Action Verbs
            KeyCode::Char(' ') | KeyCode::Char('x') => self.toggle_selected_status(now),
            KeyCode::Char('o') => self.open_selected_link(now),
            KeyCode::Char('e') => self.open_selected_in_editor(now),
            KeyCode::Char('s') => {
                self.sort_order = self.sort_order.next();
                self.clamp_indices(now);
                self.set_status(format!("Deadline sorting: {}", self.sort_order.label()));
            }
            KeyCode::Char('c') => {
                self.show_completed = !self.show_completed;
                self.clamp_indices(now);
                self.set_status(if self.show_completed {
                    "All Deadlines: Showing all items (including completed)"
                } else {
                    "All Deadlines: Hiding completed items"
                });
            }
            KeyCode::Char('a') => self.refresh_ai_summary(now),
            KeyCode::Char('L') => self.initiate_d2l_login(false),
            KeyCode::Char('S') => {
                self.sync_with_supabase();
                self.run_d2l_reconcile();
            }
            KeyCode::Char('u') => self.sync_with_supabase(),
            KeyCode::Char('R') => self.toggle_or_refresh_readings(now),
            KeyCode::Char('r') => self.reload_vault(),
            KeyCode::Char('t') => {
                self.view_all_tab = !self.view_all_tab;
                if self.view_all_tab {
                    self.active_pane = ActivePane::AllDeadlines;
                    self.set_status("Viewing Full Term Roadmap (Maximized All Deadlines)");
                } else {
                    self.set_status("Viewing Academic Cockpit");
                }
            }

            _ => {}
        }
    }

    fn handle_search_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_query.clear();
                self.clamp_indices(Local::now());
                self.mode = AppMode::Normal;
            }
            KeyCode::Enter => {
                self.mode = AppMode::Normal;
                self.clamp_indices(Local::now());
                self.set_status(format!("Filter applied: '{}'", self.search_query));
            }
            KeyCode::Backspace => {
                self.search_query.pop();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
            }
            _ => {}
        }
    }

    fn handle_command_mode(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.command_query.clear();
                self.mode = AppMode::Normal;
            }
            KeyCode::Enter => {
                let cmd = self.command_query.trim().to_string();
                self.execute_command(&cmd);
                self.command_query.clear();
                self.mode = AppMode::Normal;
            }
            KeyCode::Backspace => {
                self.command_query.pop();
            }
            KeyCode::Char(c) => {
                self.command_query.push(c);
            }
            _ => {}
        }
    }

    fn execute_command(&mut self, cmd: &str) {
        let now = Local::now();
        match cmd {
            "q" | "quit" => self.should_quit = true,
            "r" | "reload" => self.reload_vault(),
            "ai" => self.refresh_ai_summary(now),
            "login" | "d2l-login" | "auth" => self.initiate_d2l_login(false),
            "login force" | "login --force" | "d2l-login --force" => self.initiate_d2l_login(true),
            "sync" => {
                self.sync_with_supabase();
                self.run_d2l_reconcile();
            }
            "push" | "pull" => self.sync_with_supabase(),
            "reconcile" => self.run_d2l_reconcile(),
            "readings" => self.toggle_or_refresh_readings(now),
            "done" | "toggle" => self.toggle_selected_status(now),
            "open" => self.open_selected_link(now),
            "edit" => self.open_selected_in_editor(now),
            "help" => self.mode = AppMode::Help,
            "sort" => {
                self.sort_order = self.sort_order.next();
                self.clamp_indices(now);
                self.set_status(format!("Sort: {}", self.sort_order.label()));
            }
            "completed" => {
                self.show_completed = !self.show_completed;
                self.clamp_indices(now);
                self.set_status(if self.show_completed {
                    "Showing all deadlines"
                } else {
                    "Hiding completed deadlines"
                });
            }
            "all" => {
                self.selected_course_filter = None;
                self.clamp_indices(now);
                self.set_status("Filter: All Courses");
            }
            _ => self.set_status(format!("Unknown command: :{}", cmd)),
        }
    }

    fn filter_course_index(&mut self, index: usize) {
        if let Some(course) = self.courses.get(index) {
            let code = course.course_code.clone();
            let has_readings = course.has_readings();
            self.selected_course_filter = Some(code.clone());
            self.clamp_indices(Local::now());
            if has_readings {
                self.set_status(format!("Filter: {} [📖 Readings Tagged - Press 'R' to view]", code));
            } else {
                self.set_status(format!("Filter: {}", code));
            }
        }
    }

    fn move_cursor_down(&mut self, now: DateTime<Local>) {
        match self.active_pane {
            ActivePane::ImmediateFocus => {
                let len = self.immediate_focus_items(now).len();
                if len > 0 && self.focus_index < len - 1 {
                    self.focus_index += 1;
                }
            }
            ActivePane::OnTheRadar => {
                let len = self.radar_items(now).len();
                if len > 0 && self.radar_index < len - 1 {
                    self.radar_index += 1;
                }
            }
            ActivePane::AllDeadlines => {
                let len = self.all_deadlines_sorted().len();
                if len > 0 && self.all_index < len - 1 {
                    self.all_index += 1;
                }
            }
            ActivePane::AiAdvisor | ActivePane::Readings => {}
        }
    }

    fn move_cursor_up(&mut self) {
        match self.active_pane {
            ActivePane::ImmediateFocus => {
                if self.focus_index > 0 {
                    self.focus_index -= 1;
                }
            }
            ActivePane::OnTheRadar => {
                if self.radar_index > 0 {
                    self.radar_index -= 1;
                }
            }
            ActivePane::AllDeadlines => {
                if self.all_index > 0 {
                    self.all_index -= 1;
                }
            }
            ActivePane::AiAdvisor | ActivePane::Readings => {}
        }
    }

    fn jump_to_top(&mut self) {
        match self.active_pane {
            ActivePane::ImmediateFocus => self.focus_index = 0,
            ActivePane::OnTheRadar => self.radar_index = 0,
            ActivePane::AllDeadlines => self.all_index = 0,
            ActivePane::AiAdvisor | ActivePane::Readings => {}
        }
    }

    fn jump_to_bottom(&mut self, now: DateTime<Local>) {
        match self.active_pane {
            ActivePane::ImmediateFocus => {
                let len = self.immediate_focus_items(now).len();
                if len > 0 {
                    self.focus_index = len - 1;
                }
            }
            ActivePane::OnTheRadar => {
                let len = self.radar_items(now).len();
                if len > 0 {
                    self.radar_index = len - 1;
                }
            }
            ActivePane::AllDeadlines => {
                let len = self.all_deadlines_sorted().len();
                if len > 0 {
                    self.all_index = len - 1;
                }
            }
            ActivePane::AiAdvisor | ActivePane::Readings => {}
        }
    }

    fn page_down(&mut self, now: DateTime<Local>) {
        for _ in 0..5 {
            self.move_cursor_down(now);
        }
    }

    fn page_up(&mut self) {
        for _ in 0..5 {
            self.move_cursor_up();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ItemStatus, ItemType};
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_active_pane_cycling() {
        let p = ActivePane::ImmediateFocus;
        let p = p.next(false);
        assert_eq!(p, ActivePane::OnTheRadar);
        let p = p.next(false);
        assert_eq!(p, ActivePane::AllDeadlines);
        let p = p.next(false);
        assert_eq!(p, ActivePane::AiAdvisor);
        let p = p.next(false);
        assert_eq!(p, ActivePane::ImmediateFocus);

        // Cycling with readings enabled
        let p = ActivePane::AiAdvisor;
        let p = p.next(true);
        assert_eq!(p, ActivePane::Readings);
        let p = p.next(true);
        assert_eq!(p, ActivePane::ImmediateFocus);
    }

    #[test]
    fn test_deadline_sorting_closest_to_farthest() {
        let tz = FixedOffset::west_opt(4 * 3600).unwrap();
        let item1 = DeadlineItem {
            id: "1".into(),
            title: "Due Later".into(),
            item_type: ItemType::Assignment,
            due_date: tz.with_ymd_and_hms(2026, 11, 15, 23, 59, 0).unwrap(),
            weight: 20.0,
            status: ItemStatus::Todo,
            submission_platform: None,
            lead_time_days: None,
            notes: None,
            course_code: "CIS*4300".into(),
            course_name: "HCI".into(),
            course_color: "cyan".into(),
        };
        let item2 = DeadlineItem {
            id: "2".into(),
            title: "Due Soon".into(),
            item_type: ItemType::Assignment,
            due_date: tz.with_ymd_and_hms(2026, 10, 5, 23, 59, 0).unwrap(),
            weight: 10.0,
            status: ItemStatus::Todo,
            submission_platform: None,
            lead_time_days: None,
            notes: None,
            course_code: "CIS*4300".into(),
            course_name: "HCI".into(),
            course_color: "cyan".into(),
        };
        let item3 = DeadlineItem {
            id: "3".into(),
            title: "Completed Old".into(),
            item_type: ItemType::Assignment,
            due_date: tz.with_ymd_and_hms(2026, 9, 20, 23, 59, 0).unwrap(),
            weight: 5.0,
            status: ItemStatus::Completed,
            submission_platform: None,
            lead_time_days: None,
            notes: None,
            course_code: "CIS*4300".into(),
            course_name: "HCI".into(),
            course_color: "cyan".into(),
        };

        let course = Course {
            course_code: "CIS*4300".into(),
            course_name: "HCI".into(),
            tags: Vec::new(),
            instructor: None,
            color: None,
            links: None,
            policies: None,
            items: vec![item1, item2, item3],
        };

        let (bg_sender, bg_receiver) = std::sync::mpsc::channel();
        let mut app = App {
            vault_path: PathBuf::new(),
            db: Database::open_in_memory().unwrap(),
            is_syncing_supabase: false,
            courses: vec![course],
            active_pane: ActivePane::ImmediateFocus,
            focus_index: 0,
            radar_index: 0,
            all_index: 0,
            selected_course_filter: None,
            mode: AppMode::Normal,
            search_query: String::new(),
            command_query: String::new(),
            status_message: None,
            ai_summary: AiAdvisor::generate_heuristic_summary(&[], &[], Local::now()),
            should_quit: false,
            suspend_for_editor: None,
            suspend_for_d2l_login: None,
            g_pressed: false,
            view_all_tab: false,
            sort_order: DeadlineSortOrder::ClosestFirst,
            show_completed: true,
            d2l_enabled: false,
            reconcile_reports: None,
            readings_summary: None,
            show_readings_pane: false,
            bg_sender,
            bg_receiver,
            is_reconciling: false,
            is_loading_readings: false,
            is_generating_ai: false,
            spinner_frame: 0,
            pending_status_updates: HashMap::new(),
        };

        let sorted = app.all_deadlines_sorted();
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].id, "3");
        assert_eq!(sorted[1].id, "2");
        assert_eq!(sorted[2].id, "1");

        app.sort_order = DeadlineSortOrder::FarthestFirst;
        let sorted_desc = app.all_deadlines_sorted();
        assert_eq!(sorted_desc[0].id, "1");
        assert_eq!(sorted_desc[1].id, "2");
        assert_eq!(sorted_desc[2].id, "3");

        app.show_completed = false;
        let active_only = app.all_deadlines_sorted();
        assert_eq!(active_only.len(), 2);
        assert!(active_only.iter().all(|i| !i.status.is_done()));
    }

    #[test]
    fn test_app_initialization_with_real_vault() {
        let app = App::new(None);
        if let Ok(app) = app {
            assert!(app.courses.len() >= 5);
            assert!(app.d2l_enabled);
            let cts = app.courses.iter().find(|c| c.course_code == "CTS*3020");
            assert!(cts.is_some());
            assert!(cts.unwrap().has_readings());
        }
    }

    #[test]
    fn test_d2l_login_keybind_and_command() {
        let mut app = App::new(None).unwrap();
        assert_eq!(app.suspend_for_d2l_login, None);

        // Test normal mode 'L' keypress
        app.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('L'),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(app.suspend_for_d2l_login, Some(false));

        // Clear and test command mode
        app.suspend_for_d2l_login = None;
        app.execute_command("login");
        assert_eq!(app.suspend_for_d2l_login, Some(false));

        app.suspend_for_d2l_login = None;
        app.execute_command("login --force");
        assert_eq!(app.suspend_for_d2l_login, Some(true));
    }

    #[test]
    fn test_async_background_tick_and_spinner() {
        let mut app = App::new(None).unwrap();
        assert_eq!(app.spinner_frame, 0);

        app.tick();
        assert_eq!(app.spinner_frame, 1);

        // Test readings background finished event
        app.is_loading_readings = true;
        let mock_readings = crate::ai::ReadingsSummary {
            course_code: "CTS*3020".into(),
            generated_at: chrono::Local::now(),
            next_class_date: "Tuesday, Oct 6".into(),
            topic: "Digital Storytelling".into(),
            assigned_readings: vec!["Rice & Mündel (2018)".into()],
            story_summary: "Story methodologies.".into(),
            class_preparation: "Prepare 1 question.".into(),
            links: vec!["https://courselink.uoguelph.ca".into()],
        };

        app.bg_sender.send(BackgroundResult::ReadingsFinished {
            course_code: "CTS*3020".into(),
            result: Ok(mock_readings.clone()),
        }).unwrap();

        app.tick();
        assert!(!app.is_loading_readings);
        assert_eq!(app.readings_summary.as_ref().unwrap().course_code, "CTS*3020");
    }

    #[test]
    fn test_async_item_status_toggle_and_spinner() {
        let tz = FixedOffset::west_opt(4 * 3600).unwrap();
        let item = DeadlineItem {
            id: "hw1".into(),
            title: "Async Test Assignment".into(),
            item_type: ItemType::Assignment,
            due_date: tz.with_ymd_and_hms(2026, 10, 15, 23, 59, 0).unwrap(),
            weight: 15.0,
            status: ItemStatus::Todo,
            submission_platform: None,
            lead_time_days: None,
            notes: None,
            course_code: "CIS*4300".into(),
            course_name: "HCI".into(),
            course_color: "cyan".into(),
        };

        let course = Course {
            course_code: "CIS*4300".into(),
            course_name: "HCI".into(),
            tags: Vec::new(),
            instructor: None,
            color: None,
            links: None,
            policies: None,
            items: vec![item],
        };

        let (bg_sender, bg_receiver) = std::sync::mpsc::channel();
        let mut app = App {
            vault_path: PathBuf::from("/tmp/nonexistent_vault"),
            db: Database::open_in_memory().unwrap(),
            is_syncing_supabase: false,
            courses: vec![course],
            active_pane: ActivePane::ImmediateFocus,
            focus_index: 0,
            radar_index: 0,
            all_index: 0,
            selected_course_filter: None,
            mode: AppMode::Normal,
            search_query: String::new(),
            command_query: String::new(),
            status_message: None,
            ai_summary: AiAdvisor::generate_heuristic_summary(&[], &[], Local::now()),
            should_quit: false,
            suspend_for_editor: None,
            suspend_for_d2l_login: None,
            g_pressed: false,
            view_all_tab: false,
            sort_order: DeadlineSortOrder::ClosestFirst,
            show_completed: true,
            d2l_enabled: false,
            reconcile_reports: None,
            readings_summary: None,
            show_readings_pane: false,
            bg_sender,
            bg_receiver,
            is_reconciling: false,
            is_loading_readings: false,
            is_generating_ai: false,
            spinner_frame: 0,
            pending_status_updates: HashMap::new(),
        };

        let now = Local::now();
        assert!(!app.is_item_pending("CIS*4300", "hw1"));
        assert!(!app.is_any_status_pending());

        // Toggle status -> triggers async operation
        app.toggle_selected_status(now);

        // Verification 1: Item is marked pending immediately
        assert!(app.is_item_pending("CIS*4300", "hw1"));
        assert!(app.is_any_status_pending());
        let pending = app.pending_update_for("CIS*4300", "hw1").unwrap();
        assert_eq!(pending.target_status, ItemStatus::Completed);
        assert_eq!(pending.title, "Async Test Assignment");

        // Verification 2: Consecutive toggle while pending is ignored/guarded
        app.toggle_selected_status(now);
        assert_eq!(app.pending_status_updates.len(), 1);

        // Verification 3: Tick advances spinner frame
        let initial_frame = app.spinner_frame;
        app.tick();
        assert_eq!(app.spinner_frame, initial_frame + 1);

        // Verification 4: Simulate successful background save completion
        app.bg_sender.send(BackgroundResult::ItemStatusSaved {
            course_code: "CIS*4300".into(),
            item_id: "hw1".into(),
            new_status: ItemStatus::Completed,
            result: Ok(()),
        }).unwrap();

        app.tick();

        // Verification 5: Pending state is cleared and item status updated to Completed
        assert!(!app.is_item_pending("CIS*4300", "hw1"));
        assert!(!app.is_any_status_pending());
        assert_eq!(app.courses[0].items[0].status, ItemStatus::Completed);
        assert!(app.status_message.as_ref().unwrap().0.contains("Marked 'Async Test Assignment' as Completed"));
    }
}
