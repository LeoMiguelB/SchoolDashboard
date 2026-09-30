use crate::db::Database;
use crate::models::{Course, ItemStatus};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_VAULT_PATH: &str = "/home/lmb/Documents/LeeOsVault/F26";

/// Deprecated: Discovers legacy course JSON files in vault (deadlines.deprecated.json or deadline.deprecated.json).
#[allow(dead_code)]
pub fn discover_course_files(vault_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(vault_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let candidates = [
                    path.join("deadlines.json"),
                    path.join("deadlines.deprecated.json"),
                    path.join("deadline.deprecated.json"),
                ];
                for candidate in candidates {
                    if candidate.exists() && candidate.is_file() {
                        files.push(candidate);
                        break;
                    }
                }
            }
        }
    }
    files.sort();
    files
}

/// Loads courses from the unified database / SQLite cache.
/// If the database is empty and a vault path is provided, it bootstraps from deprecated vault backups.
#[allow(dead_code)]
pub fn load_courses(vault_path: &Path) -> Result<Vec<Course>> {
    let db = Database::open_default()?;
    let _ = db.bootstrap_from_vault_deprecated_if_empty(vault_path);
    db.load_courses()
}

/// Saves deadline status to the local SQLite cache and marks it for Supabase sync.
#[allow(dead_code)]
pub fn save_item_status(
    _vault_path: &Path,
    course_code: &str,
    item_id: &str,
    new_status: ItemStatus,
) -> Result<()> {
    let db = Database::open_default()?;
    db.save_item_status(course_code, item_id, new_status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_real_courses() {
        let path = Path::new(DEFAULT_VAULT_PATH);
        let courses = load_courses(path).expect("Failed to load courses");
        assert_eq!(courses.len(), 5, "Expected 5 courses");

        let codes: Vec<_> = courses.iter().map(|c| c.course_code.as_str()).collect();
        assert!(codes.contains(&"CIS*4300"));
        assert!(codes.contains(&"CTS*3020"));
        assert!(codes.contains(&"STAT*2050"));
        assert!(codes.contains(&"CIS*3090"));
        assert!(codes.contains(&"CIS*4020"));

        let total_items: usize = courses.iter().map(|c| c.items.len()).sum();
        assert!(total_items >= 25, "Expected at least 25 total deadlines across all 5 courses");
    }
}
