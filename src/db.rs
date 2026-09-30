use crate::d2l_plugin::{Discrepancy, ReconciliationReport};
use crate::models::{Course, DeadlineItem, ItemStatus, ItemType};
use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use reqwest::blocking::Client as HttpClient;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SupabaseConfig {
    pub url: String,
    pub api_key: String,
    pub publishable_key: Option<String>,
    pub secret_key: Option<String>,
    pub jwks_url: Option<String>,
}

impl SupabaseConfig {
    pub fn load() -> Option<Self> {
        let mut url = std::env::var("SUPABASE_URL").ok();
        let mut publishable_key = std::env::var("SUPABASE_PUBLISHABLE_KEY")
            .or_else(|_| std::env::var("SUPABASE_ANON_KEY"))
            .or_else(|_| std::env::var("SUPABASE_KEY"))
            .ok();
        let mut secret_key = std::env::var("SUPABASE_SECRET_KEY")
            .or_else(|_| std::env::var("SUPABASE_SERVICE_ROLE_KEY"))
            .ok();
        let mut jwks_url = std::env::var("SUPABASE_JWKS_URL").ok();

        // If not fully set from environment variables, check candidate config files
        let candidate_paths = vec![
            PathBuf::from(".env"),
            dirs::config_dir()
                .map(|p| p.join("schoodash/supabase.env"))
                .unwrap_or_default(),
            dirs::config_dir()
                .map(|p| p.join("unified_planner/supabase.env"))
                .unwrap_or_default(),
            dirs::home_dir()
                .map(|p| p.join(".supabase_planner.env"))
                .unwrap_or_default(),
        ];

        for path in candidate_paths {
            if path.exists() && path.is_file() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }
                        if let Some((k, v)) = line.split_once('=') {
                            let k = k.trim();
                            let v = v.trim().trim_matches('"').trim_matches('\'').to_string();
                            if !v.is_empty() {
                                match k {
                                    "SUPABASE_URL" => {
                                        if url.is_none() {
                                            url = Some(v);
                                        }
                                    }
                                    "SUPABASE_PUBLISHABLE_KEY" | "SUPABASE_ANON_KEY" | "SUPABASE_KEY" => {
                                        if publishable_key.is_none() {
                                            publishable_key = Some(v);
                                        }
                                    }
                                    "SUPABASE_SECRET_KEY" | "SUPABASE_SERVICE_ROLE_KEY" => {
                                        if secret_key.is_none() {
                                            secret_key = Some(v);
                                        }
                                    }
                                    "SUPABASE_JWKS_URL" => {
                                        if jwks_url.is_none() {
                                            jwks_url = Some(v);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(u) = url {
            let trimmed_u = u.trim().trim_end_matches('/').to_string();
            // Prefer secret_key if present, otherwise publishable_key
            let key = secret_key
                .as_ref()
                .cloned()
                .or_else(|| publishable_key.as_ref().cloned());

            if let Some(k) = key {
                let trimmed_k = k.trim().to_string();
                if !trimmed_u.is_empty() && !trimmed_k.is_empty() {
                    let inferred_jwks = jwks_url.or_else(|| {
                        Some(format!("{}/auth/v1/.well-known/jwks.json", trimmed_u))
                    });
                    return Some(Self {
                        url: trimmed_u,
                        api_key: trimmed_k,
                        publishable_key,
                        secret_key,
                        jwks_url: inferred_jwks,
                    });
                }
            }
        }

        None
    }
}

pub struct SupabaseClient {
    config: SupabaseConfig,
    client: HttpClient,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SupabaseCourseRow {
    pub course_code: String,
    pub course_name: String,
    #[serde(default)]
    pub instructor: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub tags: serde_json::Value,
    #[serde(default)]
    pub links: Option<serde_json::Value>,
    #[serde(default)]
    pub policies: Option<serde_json::Value>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SupabaseDeadlineRow {
    pub id: String,
    pub course_code: String,
    pub title: String,
    pub item_type: String,
    pub due_date: String,
    #[serde(default)]
    pub weight: f64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub submission_platform: Option<String>,
    #[serde(default)]
    pub lead_time_days: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl SupabaseClient {
    pub fn new(config: SupabaseConfig) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert("apikey", HeaderValue::from_str(&config.api_key)?);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", config.api_key))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let client = HttpClient::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self { config, client })
    }

    pub fn fetch_courses(&self) -> Result<Vec<SupabaseCourseRow>> {
        let endpoint = format!("{}/rest/v1/courses?select=*&order=course_code.asc", self.config.url);
        let res = self.client.get(&endpoint).send()?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().unwrap_or_default();
            anyhow::bail!("Supabase fetch courses failed ({}): {}", status, text);
        }
        let rows = res.json::<Vec<SupabaseCourseRow>>()?;
        Ok(rows)
    }

    pub fn fetch_deadlines(&self) -> Result<Vec<SupabaseDeadlineRow>> {
        let endpoint = format!("{}/rest/v1/deadlines?select=*&order=due_date.asc", self.config.url);
        let res = self.client.get(&endpoint).send()?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().unwrap_or_default();
            anyhow::bail!("Supabase fetch deadlines failed ({}): {}", status, text);
        }
        let rows = res.json::<Vec<SupabaseDeadlineRow>>()?;
        Ok(rows)
    }

    pub fn upsert_courses(&self, courses: &[SupabaseCourseRow]) -> Result<()> {
        if courses.is_empty() {
            return Ok(());
        }
        let endpoint = format!("{}/rest/v1/courses", self.config.url);
        let res = self
            .client
            .post(&endpoint)
            .header("Prefer", "resolution=merge-duplicates")
            .json(courses)
            .send()?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().unwrap_or_default();
            anyhow::bail!("Supabase upsert courses failed ({}): {}", status, text);
        }
        Ok(())
    }

    pub fn upsert_deadlines(&self, deadlines: &[SupabaseDeadlineRow]) -> Result<()> {
        if deadlines.is_empty() {
            return Ok(());
        }
        let endpoint = format!("{}/rest/v1/deadlines", self.config.url);
        let res = self
            .client
            .post(&endpoint)
            .header("Prefer", "resolution=merge-duplicates")
            .json(deadlines)
            .send()?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().unwrap_or_default();
            anyhow::bail!("Supabase upsert deadlines failed ({}): {}", status, text);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SyncSummary {
    pub pushed: usize,
    pub pulled_courses: usize,
    pub pulled_deadlines: usize,
    pub is_online: bool,
    pub message: String,
}

pub struct Database {
    conn: Connection,
    pub path_display: String,
    supabase_client: Option<SupabaseClient>,
}

impl Database {
    pub fn default_db_path() -> PathBuf {
        if let Ok(custom) = std::env::var("SCHOODASH_DB_PATH") {
            return PathBuf::from(custom);
        }
        dirs::data_local_dir()
            .map(|p| p.join("schoodash/cache.db"))
            .unwrap_or_else(|| PathBuf::from("schoodash_cache.db"))
    }

    pub fn open_default() -> Result<Self> {
        let path = Self::default_db_path();
        Self::open(&path)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn,
            path_display: ":memory:".to_string(),
            supabase_client: None,
        };
        db.init_schema()?;
        Ok(db)
    }

    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path)?;
        let path_display = path.to_string_lossy().to_string();

        let supabase_client = SupabaseConfig::load().and_then(|cfg| SupabaseClient::new(cfg).ok());

        let db = Self {
            conn,
            path_display,
            supabase_client,
        };

        db.init_schema()?;
        Ok(db)
    }

    pub fn has_supabase_configured(&self) -> bool {
        self.supabase_client.is_some()
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS courses (
                course_code TEXT PRIMARY KEY,
                course_name TEXT NOT NULL,
                instructor TEXT,
                color TEXT,
                tags_json TEXT NOT NULL DEFAULT '[]',
                links_json TEXT DEFAULT '{}',
                policies_json TEXT DEFAULT '{}',
                sync_status TEXT NOT NULL DEFAULT 'synced',
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS deadlines (
                id TEXT PRIMARY KEY,
                course_code TEXT NOT NULL,
                title TEXT NOT NULL,
                item_type TEXT NOT NULL,
                due_date TEXT NOT NULL,
                weight REAL NOT NULL DEFAULT 0.0,
                status TEXT NOT NULL DEFAULT 'todo',
                submission_platform TEXT,
                lead_time_days INTEGER,
                notes TEXT,
                sync_status TEXT NOT NULL DEFAULT 'synced',
                updated_at TEXT NOT NULL,
                FOREIGN KEY (course_code) REFERENCES courses(course_code)
            );

            CREATE INDEX IF NOT EXISTS idx_deadlines_course ON deadlines(course_code);
            CREATE INDEX IF NOT EXISTS idx_deadlines_due_date ON deadlines(due_date);
            CREATE INDEX IF NOT EXISTS idx_deadlines_sync ON deadlines(sync_status);
            ",
        )?;
        Ok(())
    }

    /// Read courses and populate initial SQLite cache from deprecated vault files if empty.
    pub fn bootstrap_from_vault_deprecated_if_empty(&self, vault_path: &Path) -> Result<usize> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM courses", [], |r| r.get(0))?;

        if count > 0 {
            return Ok(0);
        }

        let mut seeded_courses = 0;
        if let Ok(entries) = std::fs::read_dir(vault_path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    let candidates = [
                        p.join("deadlines.deprecated.json"),
                        p.join("deadline.deprecated.json"),
                        p.join("deadlines.json"),
                    ];

                    for candidate in candidates {
                        if candidate.is_file() {
                            if let Ok(content) = std::fs::read_to_string(&candidate) {
                                if let Ok(course) = serde_json::from_str::<Course>(&content) {
                                    self.upsert_local_course(&course, "pending_insert")?;
                                    seeded_courses += 1;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(seeded_courses)
    }

    pub fn upsert_local_course(&self, course: &Course, sync_status: &str) -> Result<()> {
        let now_str = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(&course.tags)?;
        let links_json = serde_json::to_string(&course.links.clone().unwrap_or_default())?;
        let policies_json = serde_json::to_string(&course.policies.clone().unwrap_or_default())?;

        self.conn.execute(
            "INSERT INTO courses (course_code, course_name, instructor, color, tags_json, links_json, policies_json, sync_status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(course_code) DO UPDATE SET
                course_name = excluded.course_name,
                instructor = excluded.instructor,
                color = excluded.color,
                tags_json = excluded.tags_json,
                links_json = excluded.links_json,
                policies_json = excluded.policies_json,
                sync_status = excluded.sync_status,
                updated_at = excluded.updated_at",
            params![
                course.course_code,
                course.course_name,
                course.instructor,
                course.color,
                tags_json,
                links_json,
                policies_json,
                sync_status,
                now_str
            ],
        )?;

        for item in &course.items {
            let item_type_str = serde_json::to_string(&item.item_type)?
                .trim_matches('"')
                .to_string();
            let status_str = serde_json::to_string(&item.status)?
                .trim_matches('"')
                .to_string();

            self.conn.execute(
                "INSERT INTO deadlines (id, course_code, title, item_type, due_date, weight, status, submission_platform, lead_time_days, notes, sync_status, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                 ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    item_type = excluded.item_type,
                    due_date = excluded.due_date,
                    weight = excluded.weight,
                    status = excluded.status,
                    submission_platform = excluded.submission_platform,
                    lead_time_days = excluded.lead_time_days,
                    notes = excluded.notes,
                    sync_status = excluded.sync_status,
                    updated_at = excluded.updated_at",
                params![
                    item.id,
                    course.course_code,
                    item.title,
                    item_type_str,
                    item.due_date.to_rfc3339(),
                    item.weight,
                    status_str,
                    item.submission_platform,
                    item.lead_time_days,
                    item.notes,
                    sync_status,
                    now_str
                ],
            )?;
        }

        Ok(())
    }

    pub fn load_courses(&self) -> Result<Vec<Course>> {
        let mut course_stmt = self.conn.prepare(
            "SELECT course_code, course_name, instructor, color, tags_json, links_json, policies_json
             FROM courses ORDER BY course_code ASC",
        )?;

        let course_rows = course_stmt.query_map([], |row| {
            let course_code: String = row.get(0)?;
            let course_name: String = row.get(1)?;
            let instructor: Option<String> = row.get(2)?;
            let color: Option<String> = row.get(3)?;
            let tags_raw: String = row.get(4)?;
            let links_raw: Option<String> = row.get(5)?;
            let policies_raw: Option<String> = row.get(6)?;

            let tags: Vec<String> = serde_json::from_str(&tags_raw).unwrap_or_default();
            let links: Option<HashMap<String, String>> =
                links_raw.and_then(|s| serde_json::from_str(&s).ok());
            let policies: Option<HashMap<String, String>> =
                policies_raw.and_then(|s| serde_json::from_str(&s).ok());

            Ok(Course {
                course_code,
                course_name,
                tags,
                instructor,
                color,
                links,
                policies,
                items: Vec::new(),
            })
        })?;

        let mut courses = Vec::new();
        for c in course_rows {
            courses.push(c?);
        }

        // Query all deadlines
        let mut item_stmt = self.conn.prepare(
            "SELECT id, course_code, title, item_type, due_date, weight, status, submission_platform, lead_time_days, notes
             FROM deadlines ORDER BY due_date ASC",
        )?;

        let item_rows = item_stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let course_code: String = row.get(1)?;
            let title: String = row.get(2)?;
            let item_type_str: String = row.get(3)?;
            let due_date_str: String = row.get(4)?;
            let weight: f64 = row.get(5)?;
            let status_str: String = row.get(6)?;
            let submission_platform: Option<String> = row.get(7)?;
            let lead_time_days: Option<i64> = row.get(8)?;
            let notes: Option<String> = row.get(9)?;

            let item_type: ItemType = serde_json::from_str(&format!("\"{}\"", item_type_str))
                .unwrap_or(ItemType::Other);
            let status: ItemStatus = serde_json::from_str(&format!("\"{}\"", status_str))
                .unwrap_or(ItemStatus::Todo);

            let due_date = DateTime::parse_from_rfc3339(&due_date_str)
                .unwrap_or_else(|_| Local::now().fixed_offset());

            Ok((
                course_code,
                DeadlineItem {
                    id,
                    title,
                    item_type,
                    due_date,
                    weight,
                    status,
                    submission_platform,
                    lead_time_days,
                    notes,
                    course_code: String::new(),
                    course_name: String::new(),
                    course_color: String::new(),
                },
            ))
        })?;

        let mut items_by_course: HashMap<String, Vec<DeadlineItem>> = HashMap::new();
        for res in item_rows {
            let (code, item) = res?;
            items_by_course.entry(code).or_default().push(item);
        }

        for course in &mut courses {
            let code = course.course_code.clone();
            let name = course.course_name.clone();
            let color = course.color.clone().unwrap_or_else(|| "blue".to_string());

            if let Some(items) = items_by_course.remove(&course.course_code) {
                for mut item in items {
                    item.course_code = code.clone();
                    item.course_name = name.clone();
                    item.course_color = color.clone();
                    course.items.push(item);
                }
            }
        }

        Ok(courses)
    }

    pub fn save_item_status(
        &self,
        _course_code: &str,
        item_id: &str,
        new_status: ItemStatus,
    ) -> Result<()> {
        let status_str = serde_json::to_string(&new_status)?
            .trim_matches('"')
            .to_string();
        let now_str = Utc::now().to_rfc3339();

        self.conn.execute(
            "UPDATE deadlines
             SET status = ?1, sync_status = 'pending_update', updated_at = ?2
             WHERE id = ?3",
            params![status_str, now_str, item_id],
        )?;

        Ok(())
    }

    pub fn apply_reconciliation(&self, reports: &[ReconciliationReport]) -> Result<usize> {
        let mut updated = 0;
        let now_str = Utc::now().to_rfc3339();

        for rep in reports {
            for disc in &rep.discrepancies {
                match disc {
                    Discrepancy::DueDateShift {
                        item_id,
                        lms_due_date,
                        ..
                    } => {
                        let due_str = lms_due_date.to_rfc3339();
                        let rows = self.conn.execute(
                            "UPDATE deadlines
                             SET due_date = ?1, sync_status = 'pending_update', updated_at = ?2
                             WHERE id = ?3",
                            params![due_str, now_str, item_id],
                        )?;
                        if rows > 0 {
                            updated += 1;
                        }
                    }
                    Discrepancy::SubmittedAutoCompleted { item_id, .. } => {
                        let rows = self.conn.execute(
                            "UPDATE deadlines
                             SET status = 'completed', sync_status = 'pending_update', updated_at = ?2
                             WHERE id = ?3 AND status != 'completed'",
                            params![now_str, item_id],
                        )?;
                        if rows > 0 {
                            updated += 1;
                        }
                    }
                    Discrepancy::MissingInVault { .. } => {
                        // Keep as discrepancy info; future creation can be added
                    }
                }
            }
        }

        Ok(updated)
    }

    pub fn get_pending_sync_count(&self) -> Result<usize> {
        let count_courses: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM courses WHERE sync_status != 'synced'",
            [],
            |r| r.get(0),
        )?;
        let count_deadlines: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM deadlines WHERE sync_status != 'synced'",
            [],
            |r| r.get(0),
        )?;
        Ok((count_courses + count_deadlines) as usize)
    }

    pub fn push_to_supabase(&self, client: &SupabaseClient) -> Result<usize> {
        let mut total_pushed = 0;

        // 1. Gather pending courses
        let mut stmt = self.conn.prepare(
            "SELECT course_code, course_name, instructor, color, tags_json, links_json, policies_json, updated_at
             FROM courses WHERE sync_status != 'synced'",
        )?;

        let course_rows = stmt.query_map([], |row| {
            let course_code: String = row.get(0)?;
            let course_name: String = row.get(1)?;
            let instructor: Option<String> = row.get(2)?;
            let color: Option<String> = row.get(3)?;
            let tags_raw: String = row.get(4)?;
            let links_raw: Option<String> = row.get(5)?;
            let policies_raw: Option<String> = row.get(6)?;
            let updated_at: String = row.get(7)?;

            let tags: serde_json::Value = serde_json::from_str(&tags_raw).unwrap_or(serde_json::json!([]));
            let links: Option<serde_json::Value> = links_raw.and_then(|s| serde_json::from_str(&s).ok());
            let policies: Option<serde_json::Value> = policies_raw.and_then(|s| serde_json::from_str(&s).ok());

            Ok(SupabaseCourseRow {
                course_code,
                course_name,
                instructor,
                color,
                tags,
                links,
                policies,
                updated_at: Some(updated_at),
            })
        })?;

        let mut courses_to_push = Vec::new();
        for r in course_rows {
            courses_to_push.push(r?);
        }

        if !courses_to_push.is_empty() {
            client.upsert_courses(&courses_to_push)?;
            for c in &courses_to_push {
                self.conn.execute(
                    "UPDATE courses SET sync_status = 'synced' WHERE course_code = ?1",
                    params![c.course_code],
                )?;
            }
            total_pushed += courses_to_push.len();
        }

        // 2. Gather pending deadlines
        let mut stmt = self.conn.prepare(
            "SELECT id, course_code, title, item_type, due_date, weight, status, submission_platform, lead_time_days, notes, updated_at
             FROM deadlines WHERE sync_status != 'synced'",
        )?;

        let deadline_rows = stmt.query_map([], |row| {
            Ok(SupabaseDeadlineRow {
                id: row.get(0)?,
                course_code: row.get(1)?,
                title: row.get(2)?,
                item_type: row.get(3)?,
                due_date: row.get(4)?,
                weight: row.get(5)?,
                status: row.get(6)?,
                submission_platform: row.get(7)?,
                lead_time_days: row.get(8)?,
                notes: row.get(9)?,
                updated_at: Some(row.get(10)?),
            })
        })?;

        let mut deadlines_to_push = Vec::new();
        for r in deadline_rows {
            deadlines_to_push.push(r?);
        }

        if !deadlines_to_push.is_empty() {
            client.upsert_deadlines(&deadlines_to_push)?;
            for d in &deadlines_to_push {
                self.conn.execute(
                    "UPDATE deadlines SET sync_status = 'synced' WHERE id = ?1",
                    params![d.id],
                )?;
            }
            total_pushed += deadlines_to_push.len();
        }

        Ok(total_pushed)
    }

    pub fn pull_from_supabase(&self, client: &SupabaseClient) -> Result<(usize, usize)> {
        let courses = client.fetch_courses()?;
        let deadlines = client.fetch_deadlines()?;

        for c in &courses {
            let tags_json = serde_json::to_string(&c.tags)?;
            let links_json = serde_json::to_string(&c.links.clone().unwrap_or(serde_json::json!({})))?;
            let policies_json = serde_json::to_string(&c.policies.clone().unwrap_or(serde_json::json!({})))?;
            let updated_at = c.updated_at.clone().unwrap_or_else(|| Utc::now().to_rfc3339());

            self.conn.execute(
                "INSERT INTO courses (course_code, course_name, instructor, color, tags_json, links_json, policies_json, sync_status, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'synced', ?8)
                 ON CONFLICT(course_code) DO UPDATE SET
                    course_name = excluded.course_name,
                    instructor = excluded.instructor,
                    color = excluded.color,
                    tags_json = excluded.tags_json,
                    links_json = excluded.links_json,
                    policies_json = excluded.policies_json,
                    sync_status = 'synced',
                    updated_at = excluded.updated_at
                 WHERE sync_status = 'synced'",
                params![
                    c.course_code,
                    c.course_name,
                    c.instructor,
                    c.color,
                    tags_json,
                    links_json,
                    policies_json,
                    updated_at
                ],
            )?;
        }

        for d in &deadlines {
            let updated_at = d.updated_at.clone().unwrap_or_else(|| Utc::now().to_rfc3339());

            self.conn.execute(
                "INSERT INTO deadlines (id, course_code, title, item_type, due_date, weight, status, submission_platform, lead_time_days, notes, sync_status, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'synced', ?11)
                 ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    item_type = excluded.item_type,
                    due_date = excluded.due_date,
                    weight = excluded.weight,
                    status = excluded.status,
                    submission_platform = excluded.submission_platform,
                    lead_time_days = excluded.lead_time_days,
                    notes = excluded.notes,
                    sync_status = 'synced',
                    updated_at = excluded.updated_at
                 WHERE sync_status = 'synced'",
                params![
                    d.id,
                    d.course_code,
                    d.title,
                    d.item_type,
                    d.due_date,
                    d.weight,
                    d.status,
                    d.submission_platform,
                    d.lead_time_days,
                    d.notes,
                    updated_at
                ],
            )?;
        }

        Ok((courses.len(), deadlines.len()))
    }

    /// Full sync: pushes pending local updates, then pulls remote updates if Supabase is connected.
    pub fn sync_full(&self) -> Result<SyncSummary> {
        if let Some(ref client) = self.supabase_client {
            let pushed = self.push_to_supabase(client)?;
            let (pulled_c, pulled_d) = self.pull_from_supabase(client)?;
            Ok(SyncSummary {
                pushed,
                pulled_courses: pulled_c,
                pulled_deadlines: pulled_d,
                is_online: true,
                message: format!(
                    "Synced with Supabase: {} pushed, {} courses & {} deadlines pulled.",
                    pushed, pulled_c, pulled_d
                ),
            })
        } else {
            let pending = self.get_pending_sync_count()?;
            Ok(SyncSummary {
                pushed: 0,
                pulled_courses: 0,
                pulled_deadlines: 0,
                is_online: false,
                message: format!(
                    "Local-only mode (offline). {} update(s) cached in SQLite.",
                    pending
                ),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_init_and_crud() {
        let temp_dir = std::env::temp_dir().join("schoodash_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join(format!("test_{}.db", uuid::Uuid::new_v4()));

        let db = Database::open(&db_path).expect("Failed to open DB");

        let course = Course {
            course_code: "TEST*1000".to_string(),
            course_name: "Test Course".to_string(),
            tags: vec!["test".to_string()],
            instructor: Some("Prof Test".to_string()),
            color: Some("blue".to_string()),
            links: None,
            policies: None,
            items: vec![DeadlineItem {
                id: "test-item-1".to_string(),
                title: "Test Assignment".to_string(),
                item_type: ItemType::Assignment,
                due_date: Local::now().fixed_offset(),
                weight: 20.0,
                status: ItemStatus::Todo,
                submission_platform: Some("CourseLink".to_string()),
                lead_time_days: Some(7),
                notes: Some("Notes here".to_string()),
                course_code: "TEST*1000".to_string(),
                course_name: "Test Course".to_string(),
                course_color: "blue".to_string(),
            }],
        };

        db.upsert_local_course(&course, "synced").expect("Upsert failed");

        let loaded = db.load_courses().expect("Load failed");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].course_code, "TEST*1000");
        assert_eq!(loaded[0].items.len(), 1);
        assert_eq!(loaded[0].items[0].id, "test-item-1");
        assert_eq!(loaded[0].items[0].status, ItemStatus::Todo);

        // Update item status
        db.save_item_status("TEST*1000", "test-item-1", ItemStatus::Completed)
            .expect("Update failed");

        let loaded2 = db.load_courses().expect("Load failed");
        assert_eq!(loaded2[0].items[0].status, ItemStatus::Completed);
        assert_eq!(db.get_pending_sync_count().unwrap(), 1);

        let _ = std::fs::remove_file(db_path);
    }

    #[test]
    fn test_bootstrap_from_vault_deprecated() {
        let temp_dir = std::env::temp_dir().join("schoodash_test");
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join(format!("test_bootstrap_{}.db", uuid::Uuid::new_v4()));

        let db = Database::open(&db_path).expect("Failed to open DB");
        let vault_path = Path::new("/home/lmb/Documents/LeeOsVault/F26");
        if vault_path.exists() {
            let seeded = db
                .bootstrap_from_vault_deprecated_if_empty(vault_path)
                .expect("Bootstrap failed");
            assert_eq!(seeded, 5, "Expected 5 courses bootstrapped from vault");

            let courses = db.load_courses().expect("Failed to load courses");
            assert_eq!(courses.len(), 5);
            let total_items: usize = courses.iter().map(|c| c.items.len()).sum();
            assert_eq!(total_items, 39, "Expected 39 total deadlines");
        }

        let _ = std::fs::remove_file(db_path);
    }
}
