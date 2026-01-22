# taill

A TUI log viewer with search, filtering, and real-time file monitoring. Watch log files as they update with syntax highlighting, level-based coloring, and powerful filtering capabilities.

## Installation

Install from crates.io:

```bash
cargo install taill
```

Or build from source:

```bash
git clone https://github.com/zhangzhishan/taill.git
cd taill
cargo build --release
```

## Usage

```bash
taill -f <pattern>
```

### Examples

Watch all log files in current directory:
```bash
taill -f "*.log"
```

Watch with a specific log format:
```bash
taill -f "*.log" --format common
```

List available log formats:
```bash
taill --list-formats
```

## Features

- **TUI Interface**: Full terminal UI with status bar, log viewer, and help panel
- **Real-time Monitoring**: Watch files as they update with automatic scrolling
- **Log Level Coloring**: Automatic detection and coloring of log levels (DEBUG/INFO/WARN/ERROR)
- **Search**: Filter logs by keyword with highlighted matches
- **Level Filtering**: Show only logs at or above a certain level
- **Pattern Matching**: Use glob syntax to watch multiple files
- **Pause/Resume**: Pause log ingestion to examine specific entries
- **Vim-style Navigation**: Familiar keybindings for scrolling
- **Configurable Log Formats**: Support for multiple log formats with custom parsing

## Log Formats

taill supports multiple log formats out of the box:

| Format | Description | Example |
|--------|-------------|---------|
| `indexserve` | IndexServe format (default) | `d,01/21/2026 17:39:52,Logger,Message` |
| `common` | Common log format | `[INFO] 2026-01-21 17:39:52 - Message` |
| `simple` | Simple format | `INFO: Message` |
| `syslog` | Syslog format | `Jan 21 17:39:52 host app: Message` |
| `json` | JSON logs | `{"level": "info", "message": "..."}` |
| `plain` | No parsing | Shows raw log lines |

Use `--format <name>` to select a format, or configure a default in the config file.

## Configuration

taill looks for configuration files in the following locations (in order):

1. `%APPDATA%\taill\config.toml` (Windows) or `~/.config/taill/config.toml` (Linux/Mac)
2. `~/.taill.toml`
3. `./taill.toml` (current directory)

### Example Configuration

```toml
# Set the default log format
default_format = "indexserve"

# Define custom log formats
[formats.myapp]
# Regex pattern with named capture groups: level, timestamp, logger, message
pattern = "^\\[(?P<level>\\w+)\\]\\s+(?P<timestamp>[^|]+)\\|\\s*(?P<message>.*)$"

# Map level strings to log levels
[formats.myapp.levels]
debug = ["DEBUG", "TRACE"]
info = ["INFO"]
warn = ["WARN", "WARNING"]
error = ["ERROR", "FATAL"]

# JSON format example
[formats.myjson]
type = "json"
level_field = "severity"
timestamp_field = "ts"
message_field = "msg"
```

### Pattern Syntax

Use regex with named capture groups:
- `(?P<level>...)` - Log level (required for level coloring)
- `(?P<timestamp>...)` - Timestamp
- `(?P<logger>...)` - Logger name
- `(?P<message>...)` - Log message

### Level Mapping

Map log level strings to standard levels:
- `debug` - Debug level (gray)
- `info` - Info level (green)
- `warn` - Warning level (yellow)
- `error` - Error level (red, bold)

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `/` | Enter search mode |
| `Enter` | Confirm search |
| `Esc` | Cancel search / Clear filter |
| `f` | Cycle level filter (ALL → INFO → WARN → ERROR) |
| `1` | Show all levels |
| `2` | Show INFO and above |
| `3` | Show WARN and above |
| `4` | Show ERROR only |
| `Space` | Pause/Resume log updates |
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `g` | Jump to top |
| `G` | Jump to bottom (follow mode) |
| `PageUp` | Scroll up 20 lines |
| `PageDown` | Scroll down 20 lines |
| `q` / `Ctrl+C` | Quit |

## Status Bar

The status bar displays:
- **Pattern**: The file pattern being watched
- **Format**: Current log format (in title)
- **Filter**: Current search query
- **Level**: Current level filter
- **Files**: Number of files being monitored
- **Lines**: Filtered lines / Total lines
- **Mode**: `[FOLLOW]` when auto-scrolling, `[PAUSED]` when paused

## Arguments

- `-f <pattern>`: The file pattern to watch (required)
- `-F, --format <name>`: Log format to use
- `--list-formats`: List available log formats
- `--help`: Display help information
- `--version`: Display version information

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Author

Zhishan Zhang (zhangzhishanlo@gmail.com)
