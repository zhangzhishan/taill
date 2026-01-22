use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::App;
use crate::log::{LogEntry, LogLevel};

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(6),
            Constraint::Length(3),
        ])
        .split(f.area());

    draw_status(f, app, chunks[0]);
    draw_logs(f, app, chunks[1]);
    draw_detail(f, app, chunks[2]);
    draw_help(f, app, chunks[3]);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let level = match app.min_level {
        LogLevel::Debug => "ALL",
        LogLevel::Info => "≥INFO",
        LogLevel::Status => "≥STATUS",
        LogLevel::Warning => "≥WARN",
        LogLevel::Error => "≥ERROR",
        LogLevel::Assert => "≥ASSERT",
        LogLevel::Event => "EVENT",
        LogLevel::Unknown => "ALL",
    };
    let mode = if app.paused {
        " [PAUSED]"
    } else if app.follow_mode {
        " [FOLLOW]"
    } else {
        ""
    };

    let status = if app.search_mode {
        format!(
            " Search: {}█  |  Level: {}  |  Files: {}  |  {}/{}",
            app.search_query, level, app.files.len(), app.filtered_indices.len(), app.logs.len()
        )
    } else {
        let filter = if app.search_query.is_empty() {
            "(none)".into()
        } else {
            format!("\"{}\"", app.search_query)
        };
        format!(
            " {}  |  Filter: {}  |  Level: {}  |  Files: {}  |  {}/{}{}",
            app.pattern_str, filter, level, app.files.len(),
            app.filtered_indices.len(), app.logs.len(), mode
        )
    };

    f.render_widget(
        Paragraph::new(status)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title(format!(" taill [{}] ", app.format_name))),
        area,
    );
}

fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let h = area.height.saturating_sub(2) as usize;
    let total = app.filtered_indices.len();

    let (start, selected_in_view) = if app.follow_mode {
        let s = total.saturating_sub(h);
        (s, total.saturating_sub(1).saturating_sub(s))
    } else {
        let s = if app.selected_index < app.scroll_offset {
            app.selected_index
        } else if app.selected_index >= app.scroll_offset + h {
            app.selected_index.saturating_sub(h) + 1
        } else {
            app.scroll_offset
        };
        (s, app.selected_index.saturating_sub(s))
    };
    let end = (start + h).min(total);

    let items: Vec<ListItem> = app.filtered_indices[start..end]
        .iter()
        .enumerate()
        .map(|(view_idx, &i)| {
            let mut item = ListItem::new(Line::from(format_entry(&app.logs[i], &app.search_query)));
            if view_idx == selected_in_view {
                item = item.style(Style::default().bg(Color::DarkGray));
            }
            item
        })
        .collect();

    f.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title(" Logs ")),
        area,
    );
}

fn format_entry(e: &LogEntry, query: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    if !e.timestamp.is_empty() {
        let t = e.timestamp.split(' ').nth(1).unwrap_or(&e.timestamp);
        spans.push(Span::styled(format!("{} ", t), Style::default().fg(Color::Cyan)));
    }

    let mut style = Style::default().fg(e.level.color());
    if e.level == LogLevel::Error {
        style = style.add_modifier(Modifier::BOLD);
    }
    spans.push(Span::styled(format!("{} ", e.level.label()), style));

    if !e.logger.is_empty() {
        spans.push(Span::styled(format!("[{}] ", e.logger), Style::default().fg(Color::Blue)));
    }

    let fname = if e.filename.len() > 20 {
        format!("...{}", &e.filename[e.filename.len() - 17..])
    } else {
        e.filename.clone()
    };
    spans.push(Span::styled(format!("<{}> ", fname), Style::default().fg(Color::Magenta)));

    if query.is_empty() {
        spans.push(Span::raw(e.message.clone()));
    } else {
        let ql = query.to_lowercase();
        let ml = e.message.to_lowercase();
        let mut last = 0;
        for (i, matched) in ml.match_indices(&ql) {
            if i > last {
                spans.push(Span::raw(e.message[last..i].to_string()));
            }
            spans.push(Span::styled(
                e.message[i..i + matched.len()].to_string(),
                Style::default().bg(Color::Yellow).fg(Color::Black),
            ));
            last = i + matched.len();
        }
        if last < e.message.len() {
            spans.push(Span::raw(e.message[last..].to_string()));
        }
    }
    spans
}

fn draw_detail(f: &mut Frame, app: &App, area: Rect) {
    let content = if let Some(entry) = app.selected_entry() {
        format!(
            "[{}] {} | {} | {}\n{}",
            entry.filename,
            entry.level.label().trim(),
            entry.timestamp,
            entry.logger,
            entry.message
        )
    } else {
        String::new()
    };

    f.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title(" Detail ")),
        area,
    );
}

fn draw_help(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.search_mode {
        " Enter: confirm | Esc: cancel | Backspace: delete "
    } else {
        " /: search | f: level | Space: pause | g/G: top/bottom | j/k: scroll | y: copy | 1-7: level | q: quit "
    };
    f.render_widget(
        Paragraph::new(text)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White))
            .block(Block::default().borders(Borders::ALL).title(" Help ")),
        area,
    );
}
