use std::collections::VecDeque;

use crate::log::{LogEntry, LogLevel};

const MAX_LOG_LINES: usize = 10000;

pub struct App {
    pub logs: VecDeque<LogEntry>,
    pub filtered_indices: Vec<usize>,
    pub scroll_offset: usize,
    pub selected_index: usize,
    pub follow_mode: bool,
    pub search_query: String,
    pub search_mode: bool,
    pub min_level: LogLevel,
    pub paused: bool,
    pub files: Vec<String>,
    pub pattern_str: String,
    pub format_name: String,
}

impl App {
    pub fn new(pattern_str: String, format_name: String) -> Self {
        Self {
            logs: VecDeque::with_capacity(MAX_LOG_LINES),
            filtered_indices: Vec::new(),
            scroll_offset: 0,
            selected_index: 0,
            follow_mode: true,
            search_query: String::new(),
            search_mode: false,
            min_level: LogLevel::Debug,
            paused: false,
            files: Vec::new(),
            pattern_str,
            format_name,
        }
    }

    pub fn add_log(&mut self, entry: LogEntry) {
        if !self.files.contains(&entry.filename) {
            self.files.push(entry.filename.clone());
        }
        if self.logs.len() >= MAX_LOG_LINES {
            self.logs.pop_front();
        }
        self.logs.push_back(entry);
        self.update_filter();
    }

    pub fn update_filter(&mut self) {
        let search_lower = self.search_query.to_lowercase();
        self.filtered_indices = self.logs.iter().enumerate()
            .filter(|(_, e)| e.level >= self.min_level)
            .filter(|(_, e)| search_lower.is_empty() || e.raw.to_lowercase().contains(&search_lower))
            .map(|(i, _)| i)
            .collect();
    }

    pub fn scroll(&mut self, delta: isize) {
        self.follow_mode = false;
        let max = self.filtered_indices.len().saturating_sub(1);
        self.selected_index = (self.selected_index as isize + delta).clamp(0, max as isize) as usize;
    }

    pub fn scroll_to(&mut self, bottom: bool) {
        self.follow_mode = bottom;
        self.selected_index = if bottom { self.filtered_indices.len().saturating_sub(1) } else { 0 };
    }

    pub fn cycle_level(&mut self) {
        self.min_level = match self.min_level {
            LogLevel::Debug => LogLevel::Info,
            LogLevel::Info => LogLevel::Status,
            LogLevel::Status => LogLevel::Warning,
            LogLevel::Warning => LogLevel::Error,
            LogLevel::Error => LogLevel::Assert,
            LogLevel::Assert => LogLevel::Event,
            _ => LogLevel::Debug,
        };
        self.update_filter();
    }

    pub fn set_level(&mut self, level: LogLevel) {
        self.min_level = level;
        self.update_filter();
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        if !self.paused {
            self.scroll_to(true);
        }
    }

    pub fn selected_entry(&self) -> Option<&LogEntry> {
        if self.follow_mode {
            self.filtered_indices.last().map(|&i| &self.logs[i])
        } else {
            self.filtered_indices.get(self.selected_index).map(|&i| &self.logs[i])
        }
    }

    pub fn click_line(&mut self, view_offset: usize, view_height: usize) {
        self.follow_mode = false;
        let total = self.filtered_indices.len();
        let start = if self.selected_index < self.scroll_offset {
            self.selected_index
        } else if self.selected_index >= self.scroll_offset + view_height {
            self.selected_index.saturating_sub(view_height) + 1
        } else {
            self.scroll_offset
        };
        let target = start + view_offset;
        if target < total {
            self.selected_index = target;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_level() {
        let mut app = App::new("*.log".into(), "test".into());
        for (lvl, msg) in [(LogLevel::Debug, "d"), (LogLevel::Info, "i"), (LogLevel::Error, "e")] {
            app.logs.push_back(LogEntry {
                raw: msg.into(),
                level: lvl,
                timestamp: String::new(),
                logger: String::new(),
                message: msg.into(),
                filename: "t.log".into(),
            });
        }
        app.update_filter();
        assert_eq!(app.filtered_indices.len(), 3);

        app.min_level = LogLevel::Info;
        app.update_filter();
        assert_eq!(app.filtered_indices.len(), 2);

        app.min_level = LogLevel::Error;
        app.update_filter();
        assert_eq!(app.filtered_indices.len(), 1);
    }

    #[test]
    fn test_filter_search() {
        let mut app = App::new("*.log".into(), "test".into());
        app.logs.push_back(LogEntry {
            raw: "hello".into(),
            level: LogLevel::Info,
            timestamp: String::new(),
            logger: String::new(),
            message: "hello".into(),
            filename: "t.log".into(),
        });
        app.logs.push_back(LogEntry {
            raw: "world".into(),
            level: LogLevel::Info,
            timestamp: String::new(),
            logger: String::new(),
            message: "world".into(),
            filename: "t.log".into(),
        });

        app.update_filter();
        assert_eq!(app.filtered_indices.len(), 2);

        app.search_query = "hello".into();
        app.update_filter();
        assert_eq!(app.filtered_indices.len(), 1);
    }
}
