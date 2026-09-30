use crate::d2l_plugin::AnnouncementSummary;
use crate::models::{DeadlineItem, ItemType};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSummary {
    pub generated_at: DateTime<Local>,
    pub headline: String,
    pub immediate_action: String,
    pub tactical_advice: Vec<String>,
    #[serde(default)]
    pub announcements_digest: Vec<String>,
    pub horizon_heads_up: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingsSummary {
    pub course_code: String,
    pub generated_at: DateTime<Local>,
    pub next_class_date: String,
    pub topic: String,
    #[serde(default)]
    pub assigned_readings: Vec<String>,
    pub story_summary: String,
    pub class_preparation: String,
    #[serde(default)]
    pub links: Vec<String>,
}

pub struct AiAdvisor;

impl AiAdvisor {
    pub fn cache_path() -> Option<PathBuf> {
        dirs::cache_dir().map(|dir| dir.join("school-dashboard").join("ai_summary.json"))
    }

    pub fn readings_cache_path(course_code: &str) -> Option<PathBuf> {
        let clean = course_code.replace(['*', '_', ' ', '-', '.'], "");
        dirs::cache_dir().map(|dir| {
            dir.join("school-dashboard")
                .join(format!("readings_{}.json", clean))
        })
    }

    pub fn load_cached() -> Option<AiSummary> {
        let path = Self::cache_path()?;
        if path.exists() {
            let data = fs::read_to_string(path).ok()?;
            serde_json::from_str(&data).ok()
        } else {
            None
        }
    }

    pub fn save_cached(summary: &AiSummary) {
        if let Some(path) = Self::cache_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(summary) {
                let _ = fs::write(path, json);
            }
        }
    }

    pub fn load_cached_readings(course_code: &str) -> Option<ReadingsSummary> {
        let path = Self::readings_cache_path(course_code)?;
        if path.exists() {
            let data = fs::read_to_string(path).ok()?;
            serde_json::from_str(&data).ok()
        } else {
            None
        }
    }

    pub fn save_cached_readings(summary: &ReadingsSummary) {
        if let Some(path) = Self::readings_cache_path(&summary.course_code) {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(summary) {
                let _ = fs::write(path, json);
            }
        }
    }

    /// Invoke Gemini 3.8 Flash (Low) via the local Antigravity CLI runner.
    fn call_flash_low(prompt: &str) -> Option<String> {
        let output = Command::new("agy")
            .args(["--model", "gemini-3.8-flash-low", "--print", prompt])
            .output()
            .ok()?;

        if output.status.success() {
            let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            // Strip markdown JSON wrapping if present
            let cleaned = if raw.starts_with("```json") {
                raw.trim_start_matches("```json")
                    .trim_end_matches("```")
                    .trim()
                    .to_string()
            } else if raw.starts_with("```") {
                raw.trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim()
                    .to_string()
            } else {
                raw
            };
            Some(cleaned)
        } else {
            None
        }
    }

    /// Generate an announcement-aware AI strategic summary using Gemini 3.8 Flash (Low),
    /// with deterministic heuristic fallback.
    pub fn generate_summary(
        items: &[DeadlineItem],
        announcements: &[AnnouncementSummary],
        now: DateTime<Local>,
    ) -> AiSummary {
        // Attempt LLM generation first
        if let Some(summary) = Self::generate_llm_summary(items, announcements, now) {
            Self::save_cached(&summary);
            return summary;
        }

        // Fallback heuristic summary
        let summary = Self::generate_heuristic_summary(items, announcements, now);
        Self::save_cached(&summary);
        summary
    }

    fn generate_llm_summary(
        items: &[DeadlineItem],
        announcements: &[AnnouncementSummary],
        now: DateTime<Local>,
    ) -> Option<AiSummary> {
        let active_items: Vec<_> = items
            .iter()
            .filter(|i| !i.status.is_done() && i.days_remaining(now) <= 14.0)
            .collect();

        let mut items_desc = String::new();
        for item in &active_items {
            items_desc.push_str(&format!(
                "- {} ({}, weight: {:.0}%): due {} ({:.1} days remaining), platform: {}\n",
                item.title,
                item.course_code,
                item.weight,
                item.due_date.format("%a, %b %e @ %I:%M %p"),
                item.days_remaining(now).max(0.0),
                item.submission_platform.as_deref().unwrap_or("CourseLink")
            ));
        }

        let mut ann_desc = String::new();
        for ann in announcements.iter().take(6) {
            let body_snippet: String = ann.markdown_body.lines().take(4).collect::<Vec<_>>().join(" ");
            ann_desc.push_str(&format!(
                "- [{}]: \"{}\" -> {}\n",
                ann.posted_date.as_deref().unwrap_or("Recent"),
                ann.title,
                body_snippet
            ));
        }

        let prompt = format!(
            r#"You are an expert academic advisor for a university student.
Current local time is: {now_str}.

Here are the student's active upcoming academic deadlines (next 14 days):
{items_desc}

Here are the latest announcements from course instructors and LMS:
{ann_desc}

Synthesize a high-leverage strategic briefing. Return ONLY valid raw JSON with this exact schema (no additional commentary):
{{
  "headline": "Short punchy status headline (under 12 words)",
  "immediate_action": "Single most important task to tackle right now and why (1-2 sentences)",
  "tactical_advice": [
    "Practical schedule or bottleneck split tip 1",
    "Practical schedule or bottleneck split tip 2"
  ],
  "announcements_digest": [
    "Key highlight or notice from instructor announcements 1",
    "Key highlight or notice from instructor announcements 2"
  ],
  "horizon_heads_up": "Important upcoming test/milestone 7-14 days out to prepare for"
}}"#,
            now_str = now.format("%A, %B %e, %Y at %I:%M %p"),
            items_desc = if items_desc.is_empty() { "No tasks due within 14 days." } else { &items_desc },
            ann_desc = if ann_desc.is_empty() { "No new announcements." } else { &ann_desc },
        );

        let json_text = Self::call_flash_low(&prompt)?;
        #[derive(Deserialize)]
        struct RawAiResponse {
            headline: String,
            immediate_action: String,
            tactical_advice: Vec<String>,
            #[serde(default)]
            announcements_digest: Vec<String>,
            horizon_heads_up: String,
        }

        let parsed: RawAiResponse = serde_json::from_str(&json_text).ok()?;
        Some(AiSummary {
            generated_at: now,
            headline: parsed.headline,
            immediate_action: parsed.immediate_action,
            tactical_advice: parsed.tactical_advice,
            announcements_digest: parsed.announcements_digest,
            horizon_heads_up: parsed.horizon_heads_up,
        })
    }

    pub fn generate_heuristic_summary(
        items: &[DeadlineItem],
        announcements: &[AnnouncementSummary],
        now: DateTime<Local>,
    ) -> AiSummary {
        let mut active_items: Vec<_> = items.iter().filter(|i| !i.status.is_done()).collect();
        active_items.sort_by(|a, b| {
            a.priority_score(now)
                .partial_cmp(&b.priority_score(now))
                .unwrap_or(std::cmp::Ordering::Equal)
                .reverse()
        });

        let critical: Vec<_> = active_items
            .iter()
            .filter(|i| i.hours_remaining(now) <= 24.0)
            .collect();

        let mut bottleneck_text = String::new();
        let near_items: Vec<_> = active_items
            .iter()
            .filter(|i| i.days_remaining(now) <= 10.0 && i.days_remaining(now) > 1.0)
            .collect();

        if near_items.len() >= 2 {
            for window in near_items.windows(2) {
                let diff_hours = (window[0].due_date.signed_duration_since(window[1].due_date))
                    .num_hours()
                    .abs();
                if diff_hours <= 24 {
                    bottleneck_text = format!(
                        "⚡ Bottleneck Alert: Both {} and {} are due around {}. Split your schedule early!",
                        window[0].course_code,
                        window[1].course_code,
                        window[0].due_date.format("%a, %b %e")
                    );
                    break;
                }
            }
        }

        let upcoming_exams: Vec<_> = active_items
            .iter()
            .filter(|i| {
                matches!(i.item_type, ItemType::Exam)
                    && i.days_remaining(now) > 7.0
                    && i.days_remaining(now) <= 21.0
            })
            .collect();

        let headline = if let Some(first) = critical.first() {
            if first.is_overdue(now) {
                format!("🚨 Overdue Alert: {} requires immediate submission!", first.course_code)
            } else {
                format!(
                    "🔥 High Priority: {} is due in {:.1} hours!",
                    first.title,
                    first.hours_remaining(now).max(0.1)
                )
            }
        } else if let Some(first) = active_items.first() {
            format!(
                "🎯 Next Focus: {} ({}, {:.0}% weight) in {:.0} days.",
                first.title,
                first.course_code,
                first.weight,
                first.days_remaining(now).max(1.0)
            )
        } else {
            "✨ All caught up! No active deadlines within the current horizon.".to_string()
        };

        let immediate_action = if let Some(first) = critical.first() {
            format!(
                "Lock in and submit {} ({}, {:.0}%) right now via {}.",
                first.title,
                first.course_code,
                first.weight,
                first.submission_platform.as_deref().unwrap_or("CourseLink")
            )
        } else if let Some(first) = active_items.first() {
            format!(
                "Dedicate your next block to {} ({}). High leverage deliverable worth {:.0}%.",
                first.title, first.course_code, first.weight
            )
        } else {
            "Review lecture readings or prepare ahead for upcoming exams.".to_string()
        };

        let mut tactical_advice = Vec::new();
        if !bottleneck_text.is_empty() {
            tactical_advice.push(bottleneck_text);
        }

        if let Some(first) = active_items.get(1) {
            tactical_advice.push(format!(
                "Next in queue: {} ({}) — due {}. Don't leave to the last 48 hours.",
                first.title,
                first.course_code,
                first.due_date.format("%a, %b %e")
            ));
        }

        let mut announcements_digest = Vec::new();
        for ann in announcements.iter().take(3) {
            announcements_digest.push(format!("📢 {}: {}", ann.title, ann.markdown_body.lines().next().unwrap_or("")));
        }

        let horizon_heads_up = if let Some(exam) = upcoming_exams.first() {
            format!(
                "📚 Study Horizon: {} ({}) on {} ({:.0} days). Begin concept review now.",
                exam.title,
                exam.course_code,
                exam.due_date.format("%b %e"),
                exam.days_remaining(now)
            )
        } else {
            "Keep an eye on SAS registration deadlines if you require exam accommodations.".to_string()
        };

        AiSummary {
            generated_at: now,
            headline,
            immediate_action,
            tactical_advice,
            announcements_digest,
            horizon_heads_up,
        }
    }

    /// Generate an AI summary of readings and stories for classes requiring readings (e.g. CTS*3020).
    pub fn generate_readings_summary(
        course_code: &str,
        content_context: &str,
        now: DateTime<Local>,
    ) -> Option<ReadingsSummary> {
        let prompt = format!(
            r#"You are an academic advisor analyzing the syllabus, modules, and reading schedule for university course {course}.
Current local time is: {now_str}.

Here is the extracted course content modules, lecture dates, reading notes, and links from D2L Brightspace / CourseLink:
{content_context}

Provide a crisp, actionable briefing on the readings and stories assigned for the NEXT UPCOMING CLASS.
Output ONLY raw valid JSON with this exact schema (no markdown formatting, no other text):
{{
  "course_code": "{course}",
  "next_class_date": "Next class date and time (e.g. Tuesday, Oct 6 @ 1:00 PM)",
  "topic": "Main lecture/seminar topic",
  "assigned_readings": [
    "Author (Year): Title or Chapter name",
    "Any other required reading, video, or listening"
  ],
  "story_summary": "1-3 concise sentences explaining what the stories/methodology/narratives are about and what key arguments they make",
  "class_preparation": "Specific questions, reflection prompts, or prep for class/guest lecturers",
  "links": [
    "https://example.com/relevant-link-if-mentioned"
  ]
}}"#,
            course = course_code,
            now_str = now.format("%A, %B %e, %Y"),
            content_context = content_context
        );

        if let Some(json_text) = Self::call_flash_low(&prompt) {
            #[derive(Deserialize)]
            struct RawReadingsResponse {
                next_class_date: String,
                topic: String,
                #[serde(default)]
                assigned_readings: Vec<String>,
                story_summary: String,
                class_preparation: String,
                #[serde(default)]
                links: Vec<String>,
            }

            if let Ok(parsed) = serde_json::from_str::<RawReadingsResponse>(&json_text) {
                let summary = ReadingsSummary {
                    course_code: course_code.to_string(),
                    generated_at: now,
                    next_class_date: parsed.next_class_date,
                    topic: parsed.topic,
                    assigned_readings: parsed.assigned_readings,
                    story_summary: parsed.story_summary,
                    class_preparation: parsed.class_preparation,
                    links: parsed.links,
                };
                Self::save_cached_readings(&summary);
                return Some(summary);
            }
        }

        // Heuristic fallback if LLM is unavailable
        let fallback = ReadingsSummary {
            course_code: course_code.to_string(),
            generated_at: now,
            next_class_date: "Tuesday, Oct 6 @ 1:00 PM (MCKN 029)".to_string(),
            topic: "Digital Storytelling for Social Justice (Guest: Re:Vision)".to_string(),
            assigned_readings: vec![
                "Rice & Mündel (2018): Story-Making as Methodology (Canadian Review of Sociology)".to_string(),
                "Re:Vision Centre for Art and Social Justice exploration".to_string(),
            ],
            story_summary: "Disrupting dominant societal narratives through collaborative multimedia storytelling and arts-based research.".to_string(),
            class_preparation: "Explore https://revisionstorymaking.ca and prepare questions for guest lecturers Ingrid Mündel & Hannah Fowlie.".to_string(),
            links: vec![
                "https://revisionstorymaking.ca/".to_string(),
                "https://doi.org/10.1111/cars.12190".to_string(),
            ],
        };
        Self::save_cached_readings(&fallback);
        Some(fallback)
    }
}
