use chrono::{DateTime, FixedOffset, Local};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Course {
    pub course_code: String,
    pub course_name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub instructor: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub links: Option<HashMap<String, String>>,
    #[serde(default)]
    pub policies: Option<HashMap<String, String>>,
    #[serde(default)]
    pub items: Vec<DeadlineItem>,
}

impl Course {
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }

    pub fn has_readings(&self) -> bool {
        self.has_tag("readings")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeadlineSortOrder {
    ClosestFirst,
    FarthestFirst,
    WeightHighest,
    CourseCode,
}

impl DeadlineSortOrder {
    pub fn label(&self) -> &'static str {
        match self {
            DeadlineSortOrder::ClosestFirst => "Closest → Farthest",
            DeadlineSortOrder::FarthestFirst => "Farthest → Closest",
            DeadlineSortOrder::WeightHighest => "Highest Weight %",
            DeadlineSortOrder::CourseCode => "Course Code",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            DeadlineSortOrder::ClosestFirst => DeadlineSortOrder::FarthestFirst,
            DeadlineSortOrder::FarthestFirst => DeadlineSortOrder::WeightHighest,
            DeadlineSortOrder::WeightHighest => DeadlineSortOrder::CourseCode,
            DeadlineSortOrder::CourseCode => DeadlineSortOrder::ClosestFirst,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    Exam,
    Assignment,
    ProjectMilestone,
    Critique,
    Presentation,
    Admin,
    Participation,
    #[serde(other)]
    Other,
}

impl ItemType {
    pub fn badge(&self) -> &'static str {
        match self {
            ItemType::Exam => "EXAM",
            ItemType::Assignment => "ASGN",
            ItemType::ProjectMilestone => "PROJ",
            ItemType::Critique => "CRIT",
            ItemType::Presentation => "PRES",
            ItemType::Admin => "ADMN",
            ItemType::Participation => "PART",
            ItemType::Other => "TASK",
        }
    }

    pub fn default_lead_time_days(&self) -> i64 {
        match self {
            ItemType::Exam => 14,
            ItemType::ProjectMilestone => 14,
            ItemType::Assignment => 12,
            ItemType::Critique => 10,
            ItemType::Presentation => 10,
            ItemType::Admin => 7,
            ItemType::Participation => 7,
            ItemType::Other => 7,
        }
    }

    pub fn priority_weight(&self) -> f64 {
        match self {
            ItemType::Exam => 1.5,
            ItemType::ProjectMilestone => 1.3,
            ItemType::Assignment => 1.1,
            ItemType::Critique => 1.1,
            ItemType::Presentation => 1.2,
            ItemType::Admin => 0.5,
            ItemType::Participation => 0.6,
            ItemType::Other => 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Todo,
    InProgress,
    Completed,
    Dropped,
}

impl ItemStatus {
    pub fn is_done(&self) -> bool {
        matches!(self, ItemStatus::Completed | ItemStatus::Dropped)
    }

    pub fn toggle(&self) -> Self {
        match self {
            ItemStatus::Todo => ItemStatus::Completed,
            ItemStatus::InProgress => ItemStatus::Completed,
            ItemStatus::Completed => ItemStatus::Todo,
            ItemStatus::Dropped => ItemStatus::Todo,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlineItem {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub item_type: ItemType,
    pub due_date: DateTime<FixedOffset>,
    #[serde(default)]
    pub weight: f64,
    #[serde(default = "default_status")]
    pub status: ItemStatus,
    #[serde(default)]
    pub submission_platform: Option<String>,
    #[serde(default)]
    pub lead_time_days: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(skip)]
    pub course_code: String,
    #[serde(skip)]
    pub course_name: String,
    #[serde(skip)]
    pub course_color: String,
}

fn default_status() -> ItemStatus {
    ItemStatus::Todo
}

impl DeadlineItem {
    pub fn effective_lead_time_days(&self) -> i64 {
        self.lead_time_days
            .unwrap_or_else(|| self.item_type.default_lead_time_days())
    }

    pub fn hours_remaining(&self, now: DateTime<Local>) -> f64 {
        let diff = self.due_date.signed_duration_since(now.fixed_offset());
        diff.num_minutes() as f64 / 60.0
    }

    pub fn days_remaining(&self, now: DateTime<Local>) -> f64 {
        self.hours_remaining(now) / 24.0
    }

    pub fn is_overdue(&self, now: DateTime<Local>) -> bool {
        !self.status.is_done() && self.hours_remaining(now) < 0.0
    }

    pub fn is_immediate_focus(&self, now: DateTime<Local>) -> bool {
        if self.status.is_done() {
            return false;
        }
        let days = self.days_remaining(now);
        // Includes overdue or due within next 7 days
        days <= 7.0
    }

    pub fn is_on_radar(&self, now: DateTime<Local>) -> bool {
        if self.status.is_done() {
            return false;
        }
        let days = self.days_remaining(now);
        let lead_limit = self.effective_lead_time_days() as f64;
        days > 7.0 && days <= lead_limit
    }

    pub fn priority_score(&self, now: DateTime<Local>) -> f64 {
        if self.status.is_done() {
            return 0.0;
        }
        let days = self.days_remaining(now).max(0.1);
        let weight_factor = if self.weight > 0.0 { self.weight } else { 2.0 };
        (weight_factor * self.item_type.priority_weight()) / days
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_horizon_filtering() {
        let now_fixed = FixedOffset::west_opt(4 * 3600).unwrap();
        let now = now_fixed.with_ymd_and_hms(2026, 9, 29, 21, 0, 0).unwrap();
        let local_now = Local.from_utc_datetime(&now.naive_utc());

        let due_soon = DeadlineItem {
            id: "t1".into(),
            title: "Test Task".into(),
            item_type: ItemType::Assignment,
            due_date: now_fixed.with_ymd_and_hms(2026, 10, 2, 23, 59, 0).unwrap(),
            weight: 15.0,
            status: ItemStatus::Todo,
            submission_platform: None,
            lead_time_days: Some(12),
            notes: None,
            course_code: "CIS*4300".into(),
            course_name: "HCI".into(),
            course_color: "cyan".into(),
        };

        assert!(due_soon.is_immediate_focus(local_now));
        assert!(!due_soon.is_on_radar(local_now));
        assert!(!due_soon.is_overdue(local_now));
    }

    #[test]
    fn test_sort_order_cycle() {
        let order = DeadlineSortOrder::ClosestFirst;
        assert_eq!(order.label(), "Closest → Farthest");
        let order = order.next();
        assert_eq!(order.label(), "Farthest → Closest");
        let order = order.next();
        assert_eq!(order.label(), "Highest Weight %");
        let order = order.next();
        assert_eq!(order.label(), "Course Code");
        let order = order.next();
        assert_eq!(order, DeadlineSortOrder::ClosestFirst);
    }

    #[test]
    fn test_course_tags() {
        let course = Course {
            course_code: "CTS*3020".into(),
            course_name: "Digital Storytelling".into(),
            tags: vec!["readings".into(), "seminar".into()],
            instructor: None,
            color: None,
            links: None,
            policies: None,
            items: Vec::new(),
        };

        assert!(course.has_tag("readings"));
        assert!(course.has_tag("READINGS"));
        assert!(course.has_readings());
        assert!(course.has_tag("seminar"));
        assert!(!course.has_tag("exams"));
    }
}

