use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_format")]
    pub default_format: String,
    #[serde(default)]
    pub formats: HashMap<String, LogFormat>,
}

fn default_format() -> String {
    "indexserve".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogFormat {
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub levels: Option<LevelMapping>,
    #[serde(rename = "type")]
    pub format_type: Option<String>,
    pub level_field: Option<String>,
    pub timestamp_field: Option<String>,
    pub message_field: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LevelMapping {
    #[serde(default)]
    pub debug: Vec<String>,
    #[serde(default)]
    pub info: Vec<String>,
    #[serde(default)]
    pub status: Vec<String>,
    #[serde(default)]
    pub warn: Vec<String>,
    #[serde(default)]
    pub error: Vec<String>,
    #[serde(default)]
    pub assert: Vec<String>,
    #[serde(default)]
    pub event: Vec<String>,
}

impl LogFormat {
    pub fn regex(pattern: &str, levels: LevelMapping) -> Self {
        Self {
            pattern: pattern.into(),
            levels: Some(levels),
            format_type: None,
            level_field: None,
            timestamp_field: None,
            message_field: None,
        }
    }

    pub fn plain(pattern: &str) -> Self {
        Self {
            pattern: pattern.into(),
            levels: None,
            format_type: None,
            level_field: None,
            timestamp_field: None,
            message_field: None,
        }
    }

    pub fn json() -> Self {
        Self {
            pattern: String::new(),
            levels: None,
            format_type: Some("json".into()),
            level_field: Some("level".into()),
            timestamp_field: Some("timestamp".into()),
            message_field: Some("message".into()),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut formats = HashMap::new();

        formats.insert(
            "indexserve".into(),
            LogFormat::regex(
                r"^(?P<level>[dDiIsSvVwWeEaA]),(?P<timestamp>[^,]+),(?P<logger>[^,]+),(?P<message>.*)$",
                LevelMapping {
                    debug: vec!["d".into(), "D".into()],
                    info: vec!["i".into(), "I".into()],
                    status: vec!["s".into(), "S".into()],
                    warn: vec!["w".into(), "W".into()],
                    error: vec!["e".into(), "E".into()],
                    assert: vec!["a".into(), "A".into()],
                    event: vec!["v".into(), "V".into()],
                },
            ),
        );

        formats.insert(
            "common".into(),
            LogFormat::regex(
                r"^\[(?P<level>DEBUG|INFO|WARN|WARNING|ERROR)\]\s+(?P<timestamp>\d{4}-\d{2}-\d{2}.\d{2}:\d{2}:\d{2})\s*-?\s*(?P<message>.*)$",
                LevelMapping {
                    debug: vec!["DEBUG".into()],
                    info: vec!["INFO".into()],
                    status: vec![],
                    warn: vec!["WARN".into(), "WARNING".into()],
                    error: vec!["ERROR".into()],
                    assert: vec![],
                    event: vec![],
                },
            ),
        );

        formats.insert(
            "simple".into(),
            LogFormat::regex(
                r"^(?P<level>DEBUG|INFO|WARN|WARNING|ERROR|TRACE):\s*(?P<message>.*)$",
                LevelMapping {
                    debug: vec!["DEBUG".into(), "TRACE".into()],
                    info: vec!["INFO".into()],
                    status: vec![],
                    warn: vec!["WARN".into(), "WARNING".into()],
                    error: vec!["ERROR".into()],
                    assert: vec![],
                    event: vec![],
                },
            ),
        );

        formats.insert(
            "syslog".into(),
            LogFormat::plain(
                r"^(?P<timestamp>\w+\s+\d+\s+[\d:]+)\s+(?P<host>\S+)\s+(?P<logger>\S+?)(?:\[\d+\])?:\s*(?P<message>.*)$",
            ),
        );

        formats.insert("json".into(), LogFormat::json());
        formats.insert("plain".into(), LogFormat::plain(r"^(?P<message>.*)$"));

        Self {
            default_format: "indexserve".into(),
            formats,
        }
    }
}

pub fn load() -> AppConfig {
    let paths = [
        dirs::config_dir().map(|d| d.join("taill/config.toml")),
        dirs::home_dir().map(|d| d.join(".taill.toml")),
        Some(PathBuf::from("taill.toml")),
    ];

    for path in paths.into_iter().flatten() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(mut config) = toml::from_str::<AppConfig>(&content) {
                for (name, format) in AppConfig::default().formats {
                    config.formats.entry(name).or_insert(format);
                }
                return config;
            }
        }
    }
    AppConfig::default()
}

pub fn list_formats(config: &AppConfig) {
    println!("Available log formats:\n");
    for (name, fmt) in &config.formats {
        let t = if fmt.format_type.as_deref() == Some("json") {
            "JSON"
        } else if fmt.pattern.is_empty() {
            "Plain"
        } else {
            "Regex"
        };
        let d = if name == &config.default_format { " (default)" } else { "" };
        println!("  {}{} - {}", name, d, t);
    }
    println!("\nUse --format <name> to select.\nConfig: ~/.config/taill/config.toml, ~/.taill.toml, or ./taill.toml");
}
