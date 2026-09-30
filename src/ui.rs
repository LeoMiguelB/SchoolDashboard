use crate::app::{ActivePane, App, AppMode};
use crate::models::{DeadlineItem, ItemStatus, ItemType};
use crate::theme::Theme;
use chrono::Local;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

pub const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();
    let now = Local::now();

    // Top-level vertical layout: Header, Tabs, Main, Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Course Tabs
            Constraint::Min(10),   // Main Cockpit
            Constraint::Length(1), // Footer / Status Bar
        ])
        .split(size);

    render_header(f, app, chunks[0], now);
    render_course_tabs(f, app, chunks[1]);

    if app.mode == AppMode::Help {
        render_help_page(f, chunks[2]);
    } else {
        render_main_cockpit(f, app, chunks[2], now);
    }

    render_footer(f, app, chunks[3]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect, now: chrono::DateTime<Local>) {
    let all_items = app.all_deadline_items();
    let overdue_count = all_items.iter().filter(|i| i.is_overdue(now)).count();
    let focus_count = all_items.iter().filter(|i| i.is_immediate_focus(now)).count();

    let title_span = Span::styled(
        " ⚡ ACADEMIC COCKPIT ",
        Style::default()
            .fg(Theme::MAUVE)
            .add_modifier(Modifier::BOLD),
    );

    let date_str = now.format("%a, %b %e, %Y  %H:%M").to_string();
    let date_span = Span::styled(format!(" 󰃭 {} ", date_str), Style::default().fg(Theme::BLUE));

    let stats_span = if overdue_count > 0 {
        Span::styled(
            format!(" [!] {} OVERDUE  •  {} In Focus ", overdue_count, focus_count),
            Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(
            format!(" • {} In Focus Window ", focus_count),
            Style::default().fg(Theme::GREEN),
        )
    };

    let spinner_char = SPINNER[app.spinner_frame % SPINNER.len()];
    let saving_span = if app.is_any_status_pending() {
        let count = app.pending_status_updates.len();
        Some(Span::styled(
            format!(" [{} Saving {} item{}...] ", spinner_char, count, if count == 1 { "" } else { "s" }),
            Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
        ))
    } else {
        None
    };

    let d2l_span = if app.is_reconciling {
        Span::styled(
            format!(" [{} D2L Syncing...] ", spinner_char),
            Style::default()
                .fg(Theme::YELLOW)
                .add_modifier(Modifier::BOLD),
        )
    } else if app.d2l_enabled {
        Span::styled(
            " [󰒋 D2L Active] ",
            Style::default()
                .fg(Theme::GREEN)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" [D2L: Off] ", Style::default().fg(Theme::MUTED))
    };

    let db_sync_span = if app.is_syncing_supabase {
        Span::styled(
            format!(" [{} Supabase Syncing...] ", spinner_char),
            Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
        )
    } else if app.db.has_supabase_configured() {
        let pending = app.db.get_pending_sync_count().unwrap_or(0);
        if pending > 0 {
            Span::styled(
                format!(" [☁ Supabase ({} pending)] ", pending),
                Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                " [☁ Supabase Synced] ",
                Style::default().fg(Theme::SKY).add_modifier(Modifier::BOLD),
            )
        }
    } else {
        Span::styled(" [💾 SQLite Cache] ", Style::default().fg(Theme::MUTED))
    };

    let mut header_spans = vec![
        title_span,
        Span::raw(" |  Fall 2026  | "),
        date_span,
        stats_span,
    ];
    if let Some(saving) = saving_span {
        header_spans.push(Span::raw(" | "));
        header_spans.push(saving);
    }
    header_spans.push(Span::raw(" | "));
    header_spans.push(d2l_span);
    header_spans.push(Span::raw(" | "));
    header_spans.push(db_sync_span);

    let header_line = Line::from(header_spans);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::OVERLAY));

    let paragraph = Paragraph::new(header_line).block(block);
    f.render_widget(paragraph, area);
}

fn render_course_tabs(f: &mut Frame, app: &App, area: Rect) {
    let mut tab_spans = Vec::new();
    let is_help_active = app.mode == AppMode::Help;

    // Tab 1: All Classes (Aggregate)
    let is_all_selected = !is_help_active && app.selected_course_filter.is_none();
    let all_style = if is_all_selected {
        Style::default()
            .fg(Theme::BASE_BG)
            .bg(Theme::LAVENDER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::SUBTEXT)
    };
    tab_spans.push(Span::styled(" [1] All Classes ", all_style));
    tab_spans.push(Span::raw(" "));

    // Tabs 2-6 for individual courses
    for (i, course) in app.courses.iter().enumerate() {
        let is_selected = !is_help_active
            && app
                .selected_course_filter
                .as_ref()
                .map(|f| f.eq_ignore_ascii_case(&course.course_code))
                .unwrap_or(false);

        let active_tasks = course.items.iter().filter(|item| !item.status.is_done()).count();
        let readings_marker = if course.has_readings() { " 📖" } else { "" };
        let label = format!(" [{}] {}{} ({}) ", i + 2, course.course_code, readings_marker, active_tasks);

        let course_color = Theme::course_color(course.color.as_deref().unwrap_or("blue"));
        let tab_style = if is_selected {
            Style::default()
                .fg(Theme::BASE_BG)
                .bg(course_color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(course_color)
        };

        tab_spans.push(Span::styled(label, tab_style));
        tab_spans.push(Span::raw(" "));
    }

    // Tab 7: Help Page
    let help_style = if is_help_active {
        Style::default()
            .fg(Theme::BASE_BG)
            .bg(Theme::GREEN)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::GREEN)
    };
    tab_spans.push(Span::styled(" [7] ❓ Help ", help_style));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::OVERLAY))
        .title(" Tabs: [1] All Classes  •  [2-6] Course Deep-Dive  •  [7/?] Help Page ");

    let paragraph = Paragraph::new(Line::from(tab_spans)).block(block);
    f.render_widget(paragraph, area);
}

fn render_main_cockpit(f: &mut Frame, app: &App, area: Rect, now: chrono::DateTime<Local>) {
    // Horizontal split: Left Pane (Tasks/Lists) vs Right Pane (AI & Inspector)
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    if app.view_all_tab {
        // Maximize All Deadlines list to fill entire left panel
        render_all_deadlines_list(f, app, h_chunks[0], now, true);
    } else {
        // Option A: Three Stacked Panes on Left
        // 1. Immediate Focus (top 28%)
        // 2. On the Radar (mid 28%)
        // 3. All Deadlines (bottom 44% - scrollable, closest to farthest)
        let v_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(28),
                Constraint::Percentage(28),
                Constraint::Percentage(44),
            ])
            .split(h_chunks[0]);

        render_focus_list(f, app, v_chunks[0], now);
        render_radar_list(f, app, v_chunks[1], now);
        render_all_deadlines_list(f, app, v_chunks[2], now, false);
    }

    // Vertical split on right: AI Advisor (top 45%) and Item Details / Readings (bottom 55%)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(h_chunks[1]);

    render_ai_advisor(f, app, right_chunks[0]);

    if app.show_readings_pane || app.active_pane == ActivePane::Readings {
        render_readings_box(f, app, right_chunks[1]);
    } else {
        render_item_details(f, app, right_chunks[1], now);
    }
}

pub fn calculate_viewport(selected: usize, total: usize, viewport_height: usize) -> (usize, usize) {
    if viewport_height == 0 || total == 0 {
        return (0, 0);
    }
    if total <= viewport_height {
        return (0, total);
    }
    let scrolloff = 2.min(viewport_height / 2);
    let mut start = 0;
    if selected >= viewport_height.saturating_sub(scrolloff) {
        start = selected + scrolloff + 1 - viewport_height;
    }
    if start + viewport_height > total {
        start = total.saturating_sub(viewport_height);
    }
    let end = (start + viewport_height).min(total);
    (start, end)
}

fn render_focus_list(f: &mut Frame, app: &App, area: Rect, now: chrono::DateTime<Local>) {
    let is_focused = app.active_pane == ActivePane::ImmediateFocus;
    let border_color = if is_focused {
        Theme::PEACH
    } else {
        Theme::OVERLAY
    };

    let items = app.immediate_focus_items(now);
    let title = if items.is_empty() {
        " 🔥 Immediate Focus (0 – 7 Days) [0 items] ".to_string()
    } else if is_focused {
        format!(
            " 🔥 Immediate Focus (0 – 7 Days) [{}/{} items] ",
            app.focus_index + 1,
            items.len()
        )
    } else {
        format!(" 🔥 Immediate Focus (0 – 7 Days) [{} items] ", items.len())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if items.is_empty() {
        let empty_msg = Paragraph::new("✨ No tasks due in the next 7 days. You are ahead of schedule!")
            .style(Style::default().fg(Theme::MUTED))
            .alignment(Alignment::Center);
        f.render_widget(empty_msg, inner);
        return;
    }

    let viewport_height = inner.height as usize;
    let (start, end) = calculate_viewport(app.focus_index, items.len(), viewport_height);

    let spinner_char = SPINNER[app.spinner_frame % SPINNER.len()];
    let mut lines = Vec::new();
    for (i, item) in items[start..end].iter().enumerate() {
        let actual_idx = start + i;
        let is_selected = is_focused && actual_idx == app.focus_index;
        let is_pending = app.is_item_pending(&item.course_code, &item.id);
        lines.push(render_item_line(item, is_selected, is_pending, spinner_char, now));
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn render_radar_list(f: &mut Frame, app: &App, area: Rect, now: chrono::DateTime<Local>) {
    let is_focused = app.active_pane == ActivePane::OnTheRadar;
    let border_color = if is_focused {
        Theme::SKY
    } else {
        Theme::OVERLAY
    };

    let items = app.radar_items(now);
    let title = if items.is_empty() {
        " 🔭 On the Radar (7 – 14 Days) [0 items] ".to_string()
    } else if is_focused {
        format!(
            " 🔭 On the Radar (7 – 14 Days) [{}/{} items] ",
            app.radar_index + 1,
            items.len()
        )
    } else {
        format!(" 🔭 On the Radar (7 – 14 Days) [{} items] ", items.len())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if items.is_empty() {
        let empty_msg = Paragraph::new("🔭 Radar clear. No major exams or milestones in the 7–14 day forecast.")
            .style(Style::default().fg(Theme::MUTED))
            .alignment(Alignment::Center);
        f.render_widget(empty_msg, inner);
        return;
    }

    let viewport_height = inner.height as usize;
    let (start, end) = calculate_viewport(app.radar_index, items.len(), viewport_height);

    let spinner_char = SPINNER[app.spinner_frame % SPINNER.len()];
    let mut lines = Vec::new();
    for (i, item) in items[start..end].iter().enumerate() {
        let actual_idx = start + i;
        let is_selected = is_focused && actual_idx == app.radar_index;
        let is_pending = app.is_item_pending(&item.course_code, &item.id);
        lines.push(render_item_line(item, is_selected, is_pending, spinner_char, now));
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn render_all_deadlines_list(
    f: &mut Frame,
    app: &App,
    area: Rect,
    now: chrono::DateTime<Local>,
    is_maximized: bool,
) {
    let is_focused = app.active_pane == ActivePane::AllDeadlines;
    let border_color = if is_focused {
        Theme::LAVENDER
    } else {
        Theme::OVERLAY
    };

    let items = app.all_deadlines_sorted();
    let sort_label = app.sort_order.label();
    let title_prefix = if is_maximized {
        " 📅 Full Term Roadmap (Maximized)"
    } else {
        " 📅 All Deadlines"
    };

    let title = if items.is_empty() {
        format!(" {} ({}) [0 items] ", title_prefix, sort_label)
    } else if is_focused {
        format!(
            " {} ({}) [{}/{} items] ",
            title_prefix,
            sort_label,
            app.all_index + 1,
            items.len()
        )
    } else {
        format!(
            " {} ({}) [{} items] ",
            title_prefix,
            sort_label,
            items.len()
        )
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if items.is_empty() {
        let empty_msg = Paragraph::new("No deadlines found matching current filter.")
            .style(Style::default().fg(Theme::MUTED))
            .alignment(Alignment::Center);
        f.render_widget(empty_msg, inner);
        return;
    }

    let viewport_height = inner.height as usize;
    let (start, end) = calculate_viewport(app.all_index, items.len(), viewport_height);

    let spinner_char = SPINNER[app.spinner_frame % SPINNER.len()];
    let mut lines = Vec::new();
    for (i, item) in items[start..end].iter().enumerate() {
        let actual_idx = start + i;
        let is_selected = is_focused && actual_idx == app.all_index;
        let is_pending = app.is_item_pending(&item.course_code, &item.id);
        lines.push(render_item_line(item, is_selected, is_pending, spinner_char, now));
    }

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn render_item_line(
    item: &DeadlineItem,
    is_selected: bool,
    is_pending: bool,
    spinner_char: &str,
    now: chrono::DateTime<Local>,
) -> Line<'static> {
    let mut spans = Vec::new();

    // Cursor indicator
    if is_selected {
        spans.push(Span::styled("▌ ", Style::default().fg(Theme::LAVENDER)));
    } else {
        spans.push(Span::raw("  "));
    }

    // Status glyph
    if is_pending {
        spans.push(Span::styled(
            format!("{} ", spinner_char),
            Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
        ));
    } else {
        let status_glyph = match item.status {
            ItemStatus::Completed => "󰄬 ",
            ItemStatus::Dropped => "󰅖 ",
            ItemStatus::InProgress => "󰄲 ",
            ItemStatus::Todo => "󰄱 ",
        };
        let status_style = match item.status {
            ItemStatus::Completed => Style::default().fg(Theme::GREEN),
            ItemStatus::Dropped => Style::default().fg(Theme::MUTED),
            ItemStatus::InProgress => Style::default().fg(Theme::YELLOW),
            ItemStatus::Todo => Style::default().fg(Theme::SUBTEXT),
        };
        spans.push(Span::styled(status_glyph, status_style));
    }

    // Urgency Icon & Badges
    if is_pending {
        spans.push(Span::styled(
            "[SAVING...] ",
            Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
        ));
    } else if item.is_overdue(now) {
        spans.push(Span::styled(" [OVERDUE] ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)));
    } else if item.hours_remaining(now) <= 24.0 && !item.status.is_done() {
        spans.push(Span::styled(" [!] ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)));
    }

    // Type Badge
    let type_badge = format!("[{}] ", item.item_type.badge());
    let type_style = match item.item_type {
        ItemType::Exam => Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD),
        ItemType::ProjectMilestone => Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD),
        ItemType::Assignment => Style::default().fg(Theme::BLUE),
        ItemType::Critique => Style::default().fg(Theme::YELLOW),
        ItemType::Presentation => Style::default().fg(Theme::SKY),
        ItemType::Admin => Style::default().fg(Theme::MUTED),
        ItemType::Participation => Style::default().fg(Theme::GREEN),
        ItemType::Other => Style::default().fg(Theme::SUBTEXT),
    };
    spans.push(Span::styled(type_badge, type_style));

    // Course Code Badge
    let course_color = Theme::course_color(&item.course_color);
    spans.push(Span::styled(
        format!("{:<8} ", item.course_code),
        Style::default().fg(course_color).add_modifier(Modifier::BOLD),
    ));

    // Title
    let title_style = if is_pending {
        Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)
    } else if item.status.is_done() {
        Style::default().fg(Theme::MUTED).add_modifier(Modifier::CROSSED_OUT)
    } else if is_selected {
        Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Theme::TEXT)
    };
    spans.push(Span::styled(item.title.clone(), title_style));

    // Relative Time Remaining (only for uncompleted items)
    if !item.status.is_done() {
        let time_str = if item.is_overdue(now) {
            format!(" ({:.1}h overdue)", item.hours_remaining(now).abs())
        } else if item.hours_remaining(now) <= 48.0 {
            format!(" ({:.1}h left)", item.hours_remaining(now))
        } else {
            format!(" ({:.0}d left)", item.days_remaining(now))
        };

        let time_style = if item.is_overdue(now) {
            Style::default().fg(Theme::RED)
        } else if item.days_remaining(now) <= 3.0 {
            Style::default().fg(Theme::PEACH)
        } else {
            Style::default().fg(Theme::MUTED)
        };
        spans.push(Span::styled(time_str, time_style));
    }

    // Weight
    if item.weight > 0.0 {
        spans.push(Span::styled(
            format!("  {:.0}%", item.weight),
            Style::default().fg(Theme::SUBTEXT),
        ));
    }

    Line::from(spans)
}

fn render_ai_advisor(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.active_pane == ActivePane::AiAdvisor;
    let border_color = if is_focused {
        Theme::MAUVE
    } else {
        Theme::OVERLAY
    };

    let spinner_char = SPINNER[app.spinner_frame % SPINNER.len()];
    let title = if app.is_generating_ai {
        format!(" 🤖 AI Strategic Advisor [{} Generating via Flash 3.8 Low...] ", spinner_char)
    } else if app.d2l_enabled {
        " 🤖 AI Strategic Advisor ('a' to refresh, Flash 3.8 Low) ".to_string()
    } else {
        " 🤖 AI Strategic Advisor ('a' to refresh) ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();
    let summary = &app.ai_summary;

    if app.is_generating_ai {
        lines.push(Line::from(vec![
            Span::styled(format!("{} ", spinner_char), Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("Synthesizing deadlines & announcements via Gemini 3.8 Flash (Low)...", Style::default().fg(Theme::YELLOW)),
        ]));
        lines.push(Line::raw(""));
    }

    // Headline
    lines.push(Line::from(Span::styled(
        &summary.headline,
        Style::default()
            .fg(Theme::YELLOW)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::raw(""));

    // Immediate Action
    lines.push(Line::from(vec![
        Span::styled("🎯 Action: ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
        Span::styled(&summary.immediate_action, Style::default().fg(Theme::TEXT)),
    ]));

    // Announcements Pulse (What's currently happening from instructor announcements)
    if !summary.announcements_digest.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "📢 Announcements & Event Pulse:",
            Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD),
        )));
        for ann in &summary.announcements_digest {
            lines.push(Line::from(vec![
                Span::styled("  • ", Style::default().fg(Theme::PEACH)),
                Span::styled(ann, Style::default().fg(Theme::SUBTEXT)),
            ]));
        }
    }

    // Tactical Advice Bullets
    if !summary.tactical_advice.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "💡 Tactical Strategy:",
            Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD),
        )));
        for advice in &summary.tactical_advice {
            lines.push(Line::from(vec![
                Span::styled("  • ", Style::default().fg(Theme::MAUVE)),
                Span::styled(advice, Style::default().fg(Theme::SUBTEXT)),
            ]));
        }
    }

    // Horizon Heads-up
    lines.push(Line::raw(""));
    lines.push(Line::from(vec![
        Span::styled("🔭 Horizon: ", Style::default().fg(Theme::SKY).add_modifier(Modifier::BOLD)),
        Span::styled(&summary.horizon_heads_up, Style::default().fg(Theme::SUBTEXT)),
    ]));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(paragraph, inner);
}

fn render_readings_box(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.active_pane == ActivePane::Readings;
    let border_color = if is_focused {
        Theme::ROSEWATER
    } else {
        Theme::OVERLAY
    };

    let spinner_char = SPINNER[app.spinner_frame % SPINNER.len()];
    let title = if app.is_loading_readings {
        format!(" 📖 Next Class Readings [{} Loading Briefing...] ", spinner_char)
    } else if let Some(ref r) = app.readings_summary {
        format!(" 📖 Next Class Readings & Story Prep ({}) [Press 'R' to close] ", r.course_code)
    } else {
        " 📖 Next Class Readings & Story Prep [Press 'R' to toggle] ".to_string()
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.is_loading_readings {
        let lines = vec![
            Line::raw(""),
            Line::from(vec![
                Span::styled(format!("  {} ", spinner_char), Style::default().fg(Theme::ROSEWATER).add_modifier(Modifier::BOLD)),
                Span::styled("Fetching CourseLink Content TOC & analyzing via Gemini 3.8 Flash (Low)...", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            ]),
            Line::raw(""),
            Line::from(Span::styled("   • Querying Brightspace module descriptions, assigned stories, and literature", Style::default().fg(Theme::SUBTEXT))),
            Line::from(Span::styled("   • Synthesizing reading requirements & discussion prep with local agy CLI runner", Style::default().fg(Theme::SUBTEXT))),
            Line::from(Span::styled("   • Dashboard remains fully interactive. Please wait a moment...", Style::default().fg(Theme::MUTED))),
        ];
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(paragraph, inner);
        return;
    }

    let r = match app.readings_summary.as_ref() {
        Some(summary) => summary,
        None => {
            let lines = vec![
                Line::raw(""),
                Line::from(Span::styled("  No readings briefing loaded for this course.", Style::default().fg(Theme::MUTED))),
                Line::from(Span::styled("  Press 'R' or type :readings to generate briefing via Gemini 3.8 Flash (Low).", Style::default().fg(Theme::SUBTEXT))),
            ];
            let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
            f.render_widget(paragraph, inner);
            return;
        }
    };

    let mut lines = Vec::new();

    // Next class & topic
    lines.push(Line::from(vec![
        Span::styled("📅 Next Class: ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
        Span::styled(&r.next_class_date, Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("🎯 Topic: ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
        Span::styled(&r.topic, Style::default().fg(Theme::TEXT)),
    ]));
    lines.push(Line::raw(""));

        // Assigned Readings
        lines.push(Line::from(Span::styled(
            "📚 Assigned Prep & Exploration:",
            Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD),
        )));
        for rd in &r.assigned_readings {
            lines.push(Line::from(vec![
                Span::styled("  • ", Style::default().fg(Theme::PEACH)),
                Span::styled(rd, Style::default().fg(Theme::TEXT)),
            ]));
        }
        lines.push(Line::raw(""));

        // Story Essence
        lines.push(Line::from(Span::styled(
            "📖 Story / Methodology Context:",
            Style::default().fg(Theme::SKY).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {}", &r.story_summary),
            Style::default().fg(Theme::SUBTEXT),
        )));
        lines.push(Line::raw(""));

        // Class prep & questions
        lines.push(Line::from(Span::styled(
            "💡 In-Class Preparation:",
            Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("  {}", &r.class_preparation),
            Style::default().fg(Theme::TEXT),
        )));

        if !r.links.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "🔗 Brightspace & External Links:",
                Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD),
            )));
            for link in &r.links {
                lines.push(Line::from(vec![
                    Span::styled("  ↳ ", Style::default().fg(Theme::SUBTEXT)),
                    Span::styled(link, Style::default().fg(Theme::BLUE)),
                ]));
            }
        }

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(paragraph, inner);
}

fn render_item_details(f: &mut Frame, app: &App, area: Rect, now: chrono::DateTime<Local>) {
    let block = Block::default()
        .title(" 📋 Item Details & Course Policies ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::OVERLAY));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if let Some(item) = app.selected_item(now) {
        let mut lines = Vec::new();
        let is_pending = app.is_item_pending(&item.course_code, &item.id);
        let spinner_char = SPINNER[app.spinner_frame % SPINNER.len()];

        let status_line = if is_pending {
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Theme::SUBTEXT)),
                Span::styled(
                    format!("{} Saving to vault...", spinner_char),
                    Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD),
                ),
            ])
        } else {
            let (status_text, status_color) = match item.status {
                ItemStatus::Completed => ("✔ Completed", Theme::GREEN),
                ItemStatus::InProgress => ("⏳ In Progress", Theme::YELLOW),
                ItemStatus::Dropped => ("✖ Dropped", Theme::MUTED),
                ItemStatus::Todo => ("To Do", Theme::SUBTEXT),
            };
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Theme::SUBTEXT)),
                Span::styled(status_text, Style::default().fg(status_color).add_modifier(Modifier::BOLD)),
            ])
        };

        lines.push(Line::from(vec![
            Span::styled("Deliverable: ", Style::default().fg(Theme::SUBTEXT)),
            Span::styled(&item.title, Style::default().fg(Theme::TEXT).add_modifier(Modifier::BOLD)),
        ]));
        lines.push(status_line);

        lines.push(Line::from(vec![
            Span::styled("Course: ", Style::default().fg(Theme::SUBTEXT)),
            Span::styled(format!("{} - {}", item.course_code, item.course_name), Style::default().fg(Theme::LAVENDER)),
        ]));

        lines.push(Line::from(vec![
            Span::styled("Due Date: ", Style::default().fg(Theme::SUBTEXT)),
            Span::styled(item.due_date.format("%A, %B %e, %Y @ %I:%M %p ET").to_string(), Style::default().fg(Theme::YELLOW)),
            Span::styled(format!("  (Weight: {:.0}%)", item.weight), Style::default().fg(Theme::GREEN)),
        ]));

        if let Some(ref platform) = item.submission_platform {
            lines.push(Line::from(vec![
                Span::styled("Submission: ", Style::default().fg(Theme::SUBTEXT)),
                Span::styled(platform, Style::default().fg(Theme::SKY)),
                Span::styled("  [Press 'o' to open LMS]", Style::default().fg(Theme::MUTED)),
            ]));
        }

        if let Some(ref notes) = item.notes {
            lines.push(Line::raw(""));
            lines.push(Line::from(vec![
                Span::styled("Notes: ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
                Span::styled(notes, Style::default().fg(Theme::TEXT)),
            ]));
        }

        // Show course policies and tags
        if let Some(course) = app.courses.iter().find(|c| c.course_code == item.course_code) {
            if !course.tags.is_empty() {
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::styled("Course Tags: ", Style::default().fg(Theme::SUBTEXT)),
                    Span::styled(course.tags.join(", "), Style::default().fg(Theme::ROSEWATER).add_modifier(Modifier::BOLD)),
                    if course.has_readings() {
                        Span::styled("  [Press 'R' for Readings Briefing]", Style::default().fg(Theme::PEACH))
                    } else {
                        Span::raw("")
                    },
                ]));
            }

            if let Some(ref policies) = course.policies {
                lines.push(Line::raw(""));
                lines.push(Line::from(Span::styled("Course Policies & Penalties:", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD))));
                for (name, val) in policies {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  • {}: ", name.replace('_', " ")), Style::default().fg(Theme::SUBTEXT)),
                        Span::styled(val, Style::default().fg(Theme::TEXT)),
                    ]));
                }
            }
        }

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
        f.render_widget(paragraph, inner);
    } else {
        let empty_msg = Paragraph::new("Select an item with 'j'/'k' to inspect policies and details.")
            .style(Style::default().fg(Theme::MUTED))
            .alignment(Alignment::Center);
        f.render_widget(empty_msg, inner);
    }
}

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let mode_str = match app.mode {
        AppMode::Normal => " NORMAL ",
        AppMode::Search => " SEARCH ",
        AppMode::Command => " COMMAND ",
        AppMode::Help => " HELP ",
    };

    let mode_style = match app.mode {
        AppMode::Normal => Style::default().fg(Theme::BASE_BG).bg(Theme::LAVENDER).add_modifier(Modifier::BOLD),
        AppMode::Search => Style::default().fg(Theme::BASE_BG).bg(Theme::PEACH).add_modifier(Modifier::BOLD),
        AppMode::Command => Style::default().fg(Theme::BASE_BG).bg(Theme::MAUVE).add_modifier(Modifier::BOLD),
        AppMode::Help => Style::default().fg(Theme::BASE_BG).bg(Theme::GREEN).add_modifier(Modifier::BOLD),
    };

    let mut spans = vec![
        Span::styled(mode_str, mode_style),
        Span::raw(" "),
    ];

    match app.mode {
        AppMode::Search => {
            spans.push(Span::styled(format!("/{}", app.search_query), Style::default().fg(Theme::TEXT)));
        }
        AppMode::Command => {
            spans.push(Span::styled(format!(":{}", app.command_query), Style::default().fg(Theme::TEXT)));
        }
        AppMode::Help => {
            spans.push(Span::styled("Interactive Help & Reference Page (Press '1' or 'Esc' to return)", Style::default().fg(Theme::GREEN)));
        }
        AppMode::Normal => {
            if let Some((ref msg, _)) = app.status_message {
                spans.push(Span::styled(msg, Style::default().fg(Theme::TEXT)));
            } else {
                let d2l_action = if app.d2l_enabled { "[L] Login  [S] Sync  [R] Readings  " } else { "" };
                spans.push(Span::styled(
                    format!("[j/k] Nav  [Space] Done  [Tab] Pane  [s] Sort  [c] Done-Filter  {}[o] Link  [e] Edit  [a] AI  [t] Max  [?] Help  [q] Quit", d2l_action),
                    Style::default().fg(Theme::MUTED)
                ));
            }
        }
    }

    let paragraph = Paragraph::new(Line::from(spans));
    f.render_widget(paragraph, area);
}

fn render_help_page(f: &mut Frame, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left Column: Keybindings & Motions
    let left_block = Block::default()
        .title(" ⌨  Vim Motions & Interactive Keymap ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::PEACH));
    let left_inner = left_block.inner(columns[0]);
    f.render_widget(left_block, columns[0]);

    let left_text = vec![
        Line::from(Span::styled("Navigation Motions:", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("  j / k       ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("Move cursor down / up by one item", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  gg / G      ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("Jump directly to top / bottom of active list", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl-d/u    ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("Half-page scroll down / up (5 items)", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  Tab / S-Tab ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("Cycle active panes (Focus ↔ Radar ↔ All Deadlines ↔ AI ↔ Readings)", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  h / l       ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("Switch between left lists and right inspector", Style::default().fg(Theme::TEXT)),
        ]),
        Line::raw(""),
        Line::from(Span::styled("Action Verbs & Keybindings:", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("  Space / x   ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("Toggle task status (Todo ⇄ Completed)", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::raw("              "),
            Span::styled("↳ Saves to local SQLite cache (syncs to Supabase on exit or 'S')", Style::default().fg(Theme::MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  L           ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("Authenticate with D2L (`d2l login`), launching browser/SSO if needed", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  S           ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
            Span::styled("Full Sync: push SQLite cache to Supabase & reconcile D2L dates", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  u           ", Style::default().fg(Theme::SAPPHIRE).add_modifier(Modifier::BOLD)),
            Span::styled("Sync with Supabase cloud connection immediately", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  R           ", Style::default().fg(Theme::ROSEWATER).add_modifier(Modifier::BOLD)),
            Span::styled("Toggle Next Class Readings & Story Briefing (Flash 3.8 Low)", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  a           ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
            Span::styled("Re-query AI Strategic Advisor with instructor announcements", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  s           ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("Cycle sort (Closest → Farthest ⇄ Farthest ⇄ Weight ⇄ Course)", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  c           ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
            Span::styled("Toggle showing/hiding completed tasks in All Deadlines", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  o           ", Style::default().fg(Theme::SKY).add_modifier(Modifier::BOLD)),
            Span::styled("Open course LMS / portal in default browser", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  e           ", Style::default().fg(Theme::TEAL).add_modifier(Modifier::BOLD)),
            Span::styled("Open course markdown notes/deadlines in $EDITOR, suspending TUI", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  t           ", Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD)),
            Span::styled("Maximize / Restore All Deadlines window (full left panel)", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  r           ", Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD)),
            Span::styled("Reload all course records from local SQLite cache", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  q / :q      ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
            Span::styled("Quit Academic Cockpit cleanly (syncs pending changes)", Style::default().fg(Theme::TEXT)),
        ]),
    ];
    let left_para = Paragraph::new(left_text);
    f.render_widget(left_para, left_inner);

    // Right Column: Navigation, Search, Commands, Storage
    let right_block = Block::default()
        .title(" 🧭  Tabs, Search, Command Mode & D2L Plugin ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::SKY));
    let right_inner = right_block.inner(columns[1]);
    f.render_widget(right_block, columns[1]);

    let right_text = vec![
        Line::from(Span::styled("Tabs & Course Hotkeys:", Style::default().fg(Theme::SKY).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("  1           ", Style::default().fg(Theme::LAVENDER).add_modifier(Modifier::BOLD)),
            Span::styled("All Classes (Consolidated Aggregate View)", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  2           ", Style::default().fg(Theme::SKY).add_modifier(Modifier::BOLD)),
            Span::styled("CIS*4300: Human-Computer Interaction", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  3           ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
            Span::styled("CTS*3020: Digital Storytelling [📖 Readings Tagged]", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  4           ", Style::default().fg(Theme::YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("STAT*2050: Statistics II", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  5           ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("CIS*3090: Parallel Programming", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  6           ", Style::default().fg(Theme::BLUE).add_modifier(Modifier::BOLD)),
            Span::styled("CIS*4020: Data Science", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  7 or ?      ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("Toggle this Interactive Reference Sheet", Style::default().fg(Theme::TEXT)),
        ]),
        Line::raw(""),
        Line::from(Span::styled("D2L Class Manager Integration:", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("  Auto-Detect ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("Checks $PATH for 'd2l'. Shows [󰒋 D2L Active] in header.", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  Reconcile   ", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD)),
            Span::styled("Syncs dropbox dates and auto-completes submitted items.", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  AI Engine   ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
            Span::styled("Powered by Gemini 3.8 Flash (Low) via agy CLI runner.", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  Readings    ", Style::default().fg(Theme::ROSEWATER).add_modifier(Modifier::BOLD)),
            Span::styled("Courses tagged 'readings' get AI summaries of next class.", Style::default().fg(Theme::TEXT)),
        ]),
        Line::raw(""),
        Line::from(Span::styled("Command Mode (:):", Style::default().fg(Theme::PEACH).add_modifier(Modifier::BOLD))),
        Line::from(vec![
            Span::styled("  :login, :auth     ", Style::default().fg(Theme::GREEN).add_modifier(Modifier::BOLD)),
            Span::styled("Authenticate with D2L Brightspace (:login force to refresh)", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  :sync, :reconcile ", Style::default().fg(Theme::SKY).add_modifier(Modifier::BOLD)),
            Span::styled("Run D2L reconciliation & update cache", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  :readings         ", Style::default().fg(Theme::ROSEWATER).add_modifier(Modifier::BOLD)),
            Span::styled("Toggle Readings & Story Briefing", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  :ai               ", Style::default().fg(Theme::MAUVE).add_modifier(Modifier::BOLD)),
            Span::styled("Refresh AI Strategic Advisor", Style::default().fg(Theme::TEXT)),
        ]),
        Line::from(vec![
            Span::styled("  :q, :quit         ", Style::default().fg(Theme::RED).add_modifier(Modifier::BOLD)),
            Span::styled("Quit application", Style::default().fg(Theme::TEXT)),
        ]),
    ];
    let right_para = Paragraph::new(right_text);
    f.render_widget(right_para, right_inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_calculate_viewport_empty_or_zero_height() {
        assert_eq!(calculate_viewport(0, 0, 10), (0, 0));
        assert_eq!(calculate_viewport(0, 10, 0), (0, 0));
    }

    #[test]
    fn test_calculate_viewport_fits_in_height() {
        assert_eq!(calculate_viewport(2, 5, 10), (0, 5));
    }

    #[test]
    fn test_calculate_viewport_scrolls_down_and_up() {
        assert_eq!(calculate_viewport(0, 20, 5), (0, 5));
        assert_eq!(calculate_viewport(2, 20, 5), (0, 5));

        let (start, end) = calculate_viewport(3, 20, 5);
        assert_eq!(end - start, 5);
        assert!(3 >= start && 3 < end);

        assert_eq!(calculate_viewport(19, 20, 5), (15, 20));
    }

    #[test]
    fn test_render_item_line_shows_loading_icon_when_pending() {
        let item = DeadlineItem {
            id: "t1".into(),
            title: "Operating Systems Assignment".into(),
            item_type: ItemType::Assignment,
            due_date: chrono::FixedOffset::west_opt(4 * 3600)
                .unwrap()
                .with_ymd_and_hms(2026, 10, 20, 23, 59, 0)
                .unwrap(),
            weight: 10.0,
            status: ItemStatus::Todo,
            submission_platform: None,
            lead_time_days: None,
            notes: None,
            course_code: "CIS*3090".into(),
            course_name: "Operating Systems".into(),
            course_color: "blue".into(),
        };

        let now = Local::now();
        let normal_line = render_item_line(&item, false, false, "⠋", now);
        let normal_text: String = normal_line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(normal_text.contains("󰄱"));
        assert!(!normal_text.contains("[SAVING...]"));

        let pending_line = render_item_line(&item, false, true, "⠋", now);
        let pending_text: String = pending_line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(pending_text.contains("⠋"));
        assert!(pending_text.contains("[SAVING...]"));
    }

    #[test]
    fn test_render_item_line_omits_time_remaining_when_completed() {
        let mut item = DeadlineItem {
            id: "t2".into(),
            title: "Completed Project".into(),
            item_type: ItemType::ProjectMilestone,
            due_date: chrono::FixedOffset::west_opt(4 * 3600)
                .unwrap()
                .with_ymd_and_hms(2026, 10, 20, 23, 59, 0)
                .unwrap(),
            weight: 15.0,
            status: ItemStatus::Completed,
            submission_platform: None,
            lead_time_days: None,
            notes: None,
            course_code: "CIS*4020".into(),
            course_name: "Data Science".into(),
            course_color: "blue".into(),
        };

        let now = Local::now();
        let line = render_item_line(&item, false, false, "⠋", now);
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!text.contains("left)"));
        assert!(!text.contains("overdue)"));

        item.status = ItemStatus::Dropped;
        let dropped_line = render_item_line(&item, false, false, "⠋", now);
        let dropped_text: String = dropped_line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(!dropped_text.contains("left)"));
        assert!(!dropped_text.contains("overdue)"));
    }
}
