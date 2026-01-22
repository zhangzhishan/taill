use regex::Regex;
use ratatui::style::Color;

use crate::config::{LevelMapping, LogFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Status = 2,
    Warning = 3,
    Error = 4,
    Assert = 5,
    Event = 6,
    Unknown = 7,
}

impl LogLevel {
    pub fn color(self) -> Color {
        match self {
            Self::Debug => Color::DarkGray,
            Self::Info => Color::Green,
            Self::Status => Color::Cyan,
            Self::Warning => Color::Yellow,
            Self::Error => Color::Red,
            Self::Assert => Color::Magenta,
            Self::Event => Color::Blue,
            Self::Unknown => Color::White,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Debug => "DEBUG ",
            Self::Info => "INFO  ",
            Self::Status => "STATUS",
            Self::Warning => "WARN  ",
            Self::Error => "ERROR ",
            Self::Assert => "ASSERT",
            Self::Event => "EVENT ",
            Self::Unknown => "??????",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub raw: String,
    pub level: LogLevel,
    pub timestamp: String,
    pub logger: String,
    pub message: String,
    pub filename: String,
}

pub struct LogParser {
    regex: Option<Regex>,
    levels: Option<LevelMapping>,
    is_json: bool,
    level_field: String,
    timestamp_field: String,
    message_field: String,
}

impl LogParser {
    pub fn new(format: &LogFormat) -> Self {
        let is_json = format.format_type.as_deref() == Some("json");
        Self {
            regex: if !is_json && !format.pattern.is_empty() {
                Regex::new(&format.pattern).ok()
            } else {
                None
            },
            levels: format.levels.clone(),
            is_json,
            level_field: format.level_field.clone().unwrap_or_else(|| "level".into()),
            timestamp_field: format.timestamp_field.clone().unwrap_or_else(|| "timestamp".into()),
            message_field: format.message_field.clone().unwrap_or_else(|| "message".into()),
        }
    }

    pub fn parse(&self, raw: &str, filename: &str) -> LogEntry {
        let content = self.strip_filename_prefix(raw, filename);

        if self.is_json {
            return self.parse_json(content, raw, filename);
        }

        if let Some(ref re) = self.regex {
            if let Some(caps) = re.captures(content) {
                let get = |name| caps.name(name).map(|m| m.as_str().to_string()).unwrap_or_default();
                let level_str = caps.name("level").map(|m| m.as_str()).unwrap_or("");
                return LogEntry {
                    raw: raw.into(),
                    level: self.map_level(level_str),
                    timestamp: get("timestamp"),
                    logger: get("logger"),
                    message: caps.name("message").map(|m| m.as_str()).unwrap_or(content).into(),
                    filename: filename.into(),
                };
            }
        }

        LogEntry {
            raw: raw.into(),
            level: LogLevel::Unknown,
            timestamp: String::new(),
            logger: String::new(),
            message: content.into(),
            filename: filename.into(),
        }
    }

    fn strip_filename_prefix<'a>(&self, raw: &'a str, filename: &str) -> &'a str {
        if let Some(rest) = raw.strip_prefix('[') {
            if let Some(end) = rest.find("] ") {
                let prefix = &rest[..end];
                if prefix.contains('.') || prefix == filename {
                    return &rest[end + 2..];
                }
            }
        }
        raw
    }

    fn parse_json(&self, content: &str, raw: &str, filename: &str) -> LogEntry {
        let extract = |field: &str| -> Option<String> {
            let pattern = format!(r#""{}":\s*"([^"]*)""#, regex::escape(field));
            Regex::new(&pattern).ok()?.captures(content)?.get(1).map(|m| m.as_str().into())
        };

        LogEntry {
            raw: raw.into(),
            level: extract(&self.level_field).map(|s| self.map_level(&s)).unwrap_or(LogLevel::Unknown),
            timestamp: extract(&self.timestamp_field).unwrap_or_default(),
            logger: String::new(),
            message: extract(&self.message_field).unwrap_or_else(|| content.into()),
            filename: filename.into(),
        }
    }

    fn map_level(&self, s: &str) -> LogLevel {
        if let Some(ref levels) = self.levels {
            if levels.debug.iter().any(|x| x.eq_ignore_ascii_case(s)) { return LogLevel::Debug; }
            if levels.info.iter().any(|x| x.eq_ignore_ascii_case(s)) { return LogLevel::Info; }
            if levels.status.iter().any(|x| x.eq_ignore_ascii_case(s)) { return LogLevel::Status; }
            if levels.warn.iter().any(|x| x.eq_ignore_ascii_case(s)) { return LogLevel::Warning; }
            if levels.error.iter().any(|x| x.eq_ignore_ascii_case(s)) { return LogLevel::Error; }
            if levels.assert.iter().any(|x| x.eq_ignore_ascii_case(s)) { return LogLevel::Assert; }
            if levels.event.iter().any(|x| x.eq_ignore_ascii_case(s)) { return LogLevel::Event; }
        }
        match s.to_uppercase().as_str() {
            "DEBUG" | "TRACE" | "D" => LogLevel::Debug,
            "INFO" | "I" => LogLevel::Info,
            "STATUS" | "S" => LogLevel::Status,
            "WARN" | "WARNING" | "W" => LogLevel::Warning,
            "ERROR" | "ERR" | "E" => LogLevel::Error,
            "ASSERT" | "FATAL" | "CRITICAL" | "A" => LogLevel::Assert,
            "EVENT" | "V" => LogLevel::Event,
            _ => LogLevel::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn test_parser_indexserve() {
        let parser = LogParser::new(AppConfig::default().formats.get("indexserve").unwrap());
        let e = parser.parse("d,01/21/2026 17:39:52,IndexQueryLog,Some message", "test.log");
        assert_eq!(e.level, LogLevel::Debug);
        assert_eq!(e.timestamp, "01/21/2026 17:39:52");
        assert_eq!(e.logger, "IndexQueryLog");
    }

    #[test]
    fn test_parser_common() {
        let parser = LogParser::new(AppConfig::default().formats.get("common").unwrap());
        assert_eq!(parser.parse("[INFO] 2026-01-21 17:39:52 - Started", "a.log").level, LogLevel::Info);
        assert_eq!(parser.parse("[ERROR] 2026-01-21T17:39:52 - Failed", "a.log").level, LogLevel::Error);
    }

    #[test]
    fn test_parser_simple() {
        let parser = LogParser::new(AppConfig::default().formats.get("simple").unwrap());
        assert_eq!(parser.parse("DEBUG: msg", "a.log").level, LogLevel::Debug);
        assert_eq!(parser.parse("ERROR: msg", "a.log").level, LogLevel::Error);
    }

    #[test]
    fn test_parser_filename_prefix() {
        let parser = LogParser::new(AppConfig::default().formats.get("indexserve").unwrap());
        let e = parser.parse("[test.log] d,01/21/2026 17:39:52,Log,Msg", "test.log");
        assert_eq!(e.level, LogLevel::Debug);
    }
}
