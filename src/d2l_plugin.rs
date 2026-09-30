use anyhow::Result;
use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PulledContentFile {
    pub course_code: String,
    pub topic_title: String,
    pub module_title: String,
    pub filename: String,
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationReport {
    pub course_code: String,
    pub org_unit_id: i64,
    pub checked_at: DateTime<Utc>,
    pub discrepancies: Vec<Discrepancy>,
    pub announcements_for_ai: Vec<AnnouncementSummary>,
    #[serde(default)]
    pub pulled_content: Vec<PulledContentFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "details")]
pub enum Discrepancy {
    DueDateShift {
        item_id: String,
        item_title: String,
        vault_due_date: DateTime<FixedOffset>,
        lms_due_date: DateTime<FixedOffset>,
    },
    SubmittedAutoCompleted {
        item_id: String,
        item_title: String,
        submission_time: String,
        file_count: usize,
    },
    MissingInVault {
        lms_folder_id: i64,
        lms_title: String,
        lms_due_date: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnouncementSummary {
    pub id: i64,
    pub title: String,
    pub posted_date: Option<String>,
    pub markdown_body: String,
}

pub struct D2lPlugin;

impl D2lPlugin {
    /// Detect if the `d2l` CLI binary is available on the user's system PATH.
    pub fn is_available() -> bool {
        Command::new("d2l")
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Run reconciliation between D2L live state and local vault `deadlines.json`.
    /// When `apply` is true, updates the deadlines.json cache with .bak backups.
    pub fn run_reconcile(apply: bool, vault_path: Option<&std::path::Path>) -> Result<Vec<ReconciliationReport>> {
        let mut cmd = Command::new("d2l");
        cmd.arg("reconcile").arg("--json");
        if apply {
            cmd.arg("--apply");
        }
        if let Some(v_path) = vault_path {
            cmd.arg("--vault").arg(v_path);
        }

        let output = cmd.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("d2l reconcile failed: {}", stderr.trim());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let reports: Vec<ReconciliationReport> = serde_json::from_str(&stdout)?;
        Ok(reports)
    }

    /// Fetch course content TOC modules and readings from D2L Brightspace.
    pub fn fetch_course_content(course_code: &str) -> Result<serde_json::Value> {
        let output = Command::new("d2l")
            .args(["content", "-c", course_code, "--json"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("d2l content failed for {}: {}", course_code, stderr.trim());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let val: serde_json::Value = serde_json::from_str(&stdout)?;
        Ok(val)
    }

    /// Extract readable context from content JSON for AI prompt ingestion.
    pub fn extract_readings_context(course_code: &str, content_json: &serde_json::Value) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Course: {}", course_code));

        if let Some(arr) = content_json.as_array() {
            for entry in arr {
                if let Some(toc) = entry.get(2) {
                    if let Some(modules) = toc.get("Modules").and_then(|m| m.as_array()) {
                        for m in modules {
                            let title = m.get("Title").and_then(|t| t.as_str()).unwrap_or("");
                            let desc = m.get("Description")
                                .and_then(|d| d.get("Text"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("");

                            if !desc.trim().is_empty() || title.to_lowercase().contains("week") {
                                lines.push(format!("\n### Module: {}", title));
                                if !desc.trim().is_empty() {
                                    lines.push(format!("Description/Readings: {}", desc.trim()));
                                }
                            }

                            if let Some(topics) = m.get("Topics").and_then(|t| t.as_array()) {
                                for t in topics {
                                    let t_title = t.get("Title").and_then(|s| s.as_str()).unwrap_or("");
                                    let t_url = t.get("Url").and_then(|s| s.as_str()).unwrap_or("");
                                    if !t_url.is_empty() {
                                        lines.push(format!("- Attachment/Topic: {} ({})", t_title, t_url));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        lines.join("\n")
    }

    /// Compute statistics on discrepancies: (shifts, autocompleted, missing_in_vault)
    pub fn discrepancy_stats(reports: &[ReconciliationReport]) -> (usize, usize, usize) {
        let mut shifts = 0;
        let mut autocompleted = 0;
        let mut missing = 0;

        for rep in reports {
            for disc in &rep.discrepancies {
                match disc {
                    Discrepancy::DueDateShift { .. } => shifts += 1,
                    Discrepancy::SubmittedAutoCompleted { .. } => autocompleted += 1,
                    Discrepancy::MissingInVault { .. } => missing += 1,
                }
            }
        }

        (shifts, autocompleted, missing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};

    #[test]
    fn test_d2l_plugin_availability() {
        // d2l is installed in ~/.cargo/bin/d2l
        assert!(D2lPlugin::is_available());
    }

    #[test]
    fn test_discrepancy_stats() {
        let tz = FixedOffset::west_opt(4 * 3600).unwrap();
        let due = tz.with_ymd_and_hms(2026, 10, 9, 23, 59, 0).unwrap();

        let report = ReconciliationReport {
            course_code: "CIS*4300".into(),
            org_unit_id: 1073361,
            checked_at: Utc::now(),
            discrepancies: vec![
                Discrepancy::DueDateShift {
                    item_id: "m1".into(),
                    item_title: "Milestone 1".into(),
                    vault_due_date: due,
                    lms_due_date: due,
                },
                Discrepancy::SubmittedAutoCompleted {
                    item_id: "a1".into(),
                    item_title: "Assignment 1".into(),
                    submission_time: "2026-10-01".into(),
                    file_count: 1,
                },
                Discrepancy::MissingInVault {
                    lms_folder_id: 12345,
                    lms_title: "Quiz 1".into(),
                    lms_due_date: None,
                },
            ],
            announcements_for_ai: Vec::new(),
            pulled_content: Vec::new(),
        };

        let (shifts, auto, missing) = D2lPlugin::discrepancy_stats(&[report]);
        assert_eq!(shifts, 1);
        assert_eq!(auto, 1);
        assert_eq!(missing, 1);
    }
}
