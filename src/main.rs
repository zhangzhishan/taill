mod app;
mod config;
mod log;
mod ui;
mod watcher;

use arboard::Clipboard;
use clap::{Arg, Command};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use glob::Pattern;
use notify::{Config as NotifyConfig, EventKind, PollWatcher, RecursiveMode, Watcher};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    collections::HashMap,
    env, fs,
    io::{self, Stdout},
    path::PathBuf,
    sync::{mpsc::channel, Arc, Mutex},
    thread,
    time::Duration,
};

use app::App;
use config::LogFormat;
use log::{LogLevel, LogParser};
use watcher::{follow_file, matches_pattern, open_file_shared, Signal};

fn run_app(
    term: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut App,
    rx: std::sync::mpsc::Receiver<String>,
    parser: Arc<LogParser>,
    clipboard: &mut Option<Clipboard>,
) -> io::Result<()> {
    loop {
        while let Ok(msg) = rx.try_recv() {
            if !app.paused {
                if let Some((fname, line)) = msg.strip_prefix('\x00').and_then(|s| s.split_once('\x00')) {
                    app.add_log(parser.parse(line, fname));
                }
            }
        }

        term.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if app.search_mode {
                        match key.code {
                            KeyCode::Enter => {
                                app.search_mode = false;
                                app.update_filter();
                            }
                            KeyCode::Esc => {
                                app.search_mode = false;
                                app.search_query.clear();
                                app.update_filter();
                            }
                            KeyCode::Backspace => {
                                app.search_query.pop();
                                app.update_filter();
                            }
                            KeyCode::Char(c) => {
                                app.search_query.push(c);
                                app.update_filter();
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(())
                            }
                            KeyCode::Char('/') => app.search_mode = true,
                            KeyCode::Char('f') => app.cycle_level(),
                            KeyCode::Char(' ') => app.toggle_pause(),
                            KeyCode::Char('g') => app.scroll_to(false),
                            KeyCode::Char('G') => app.scroll_to(true),
                            KeyCode::Char('j') | KeyCode::Down => app.scroll(1),
                            KeyCode::Char('k') | KeyCode::Up => app.scroll(-1),
                            KeyCode::PageDown => app.scroll(20),
                            KeyCode::PageUp => app.scroll(-20),
                            KeyCode::Esc => {
                                app.search_query.clear();
                                app.update_filter();
                            }
                            KeyCode::Char('1') => app.set_level(LogLevel::Debug),
                            KeyCode::Char('2') => app.set_level(LogLevel::Info),
                            KeyCode::Char('3') => app.set_level(LogLevel::Status),
                            KeyCode::Char('4') => app.set_level(LogLevel::Warning),
                            KeyCode::Char('5') => app.set_level(LogLevel::Error),
                            KeyCode::Char('6') => app.set_level(LogLevel::Assert),
                            KeyCode::Char('7') => app.set_level(LogLevel::Event),
                            KeyCode::Char('y') => {
                                if let Some(entry) = app.selected_entry() {
                                    if let Some(cb) = clipboard.as_mut() {
                                        let _ = cb.set_text(entry.raw.clone());
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::ScrollUp => app.scroll(-3),
                        MouseEventKind::ScrollDown => app.scroll(3),
                        MouseEventKind::Down(_) => {
                            let term_height = term.size()?.height;
                            // Layout: status(3) + logs(min 1) + detail(6) + help(3)
                            let logs_start = 3;
                            let logs_end = term_height.saturating_sub(9);
                            let logs_height = (logs_end - logs_start).saturating_sub(2) as usize;
                            let row = mouse.row;
                            if row > logs_start && row < logs_end {
                                let click_offset = (row - logs_start - 1) as usize;
                                app.click_line(click_offset, logs_height);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = config::load();

    let matches = Command::new("taill")
        .version(env!("CARGO_PKG_VERSION"))
        .about("TUI log viewer with search and filtering")
        .arg(
            Arg::new("pattern")
                .short('f')
                .help("File pattern to watch")
                .required_unless_present("list_formats"),
        )
        .arg(Arg::new("format").short('F').long("format").help("Log format"))
        .arg(
            Arg::new("list_formats")
                .long("list-formats")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    if matches.get_flag("list_formats") {
        config::list_formats(&cfg);
        return Ok(());
    }

    let pattern_str = matches.get_one::<String>("pattern").unwrap().clone();
    let format_name = matches
        .get_one::<String>("format")
        .cloned()
        .unwrap_or_else(|| cfg.default_format.clone());

    let log_format = cfg.formats.get(&format_name).cloned().unwrap_or_else(|| {
        eprintln!("Warning: Unknown format '{}', using 'plain'", format_name);
        LogFormat::plain(r"^(?P<message>.*)$")
    });

    let parser = Arc::new(LogParser::new(&log_format));
    let pattern = Pattern::new(&pattern_str)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

    let mut app = App::new(pattern_str.clone(), format_name);
    let (tx, rx) = channel::<String>();

    let current_dir = env::current_dir()?;
    let folder = PathBuf::from(&pattern_str)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                current_dir.join(p)
            }
        })
        .unwrap_or_else(|| current_dir.clone());

    let (wtx, wrx) = channel();
    let mut watcher: PollWatcher = PollWatcher::new(
        wtx,
        NotifyConfig::default()
            .with_poll_interval(Duration::from_millis(500))
            .with_compare_contents(true),
    )?;
    watcher.watch(&folder, RecursiveMode::NonRecursive)?;

    let files: Arc<Mutex<HashMap<String, std::sync::mpsc::Sender<Signal>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Scan existing files
    for entry in fs::read_dir(&folder)?.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(fname) = matches_pattern(&path, &pattern, &current_dir) {
                if let Ok(file) = open_file_shared(&path) {
                    let (stx, srx) = channel();
                    files.lock().unwrap().insert(fname.clone(), stx);
                    let tx = tx.clone();
                    let srx = Arc::new(Mutex::new(srx));
                    thread::spawn(move || follow_file(file, srx, false, fname, tx));
                }
            }
        }
    }

    // Watcher thread
    let files2 = Arc::clone(&files);
    let tx2 = tx.clone();
    let pattern2 = pattern.clone();
    let cdir2 = current_dir.clone();
    thread::spawn(move || {
        while let Ok(Ok(ev)) = wrx.recv() {
            for path in ev.paths {
                if let Some(fname) = matches_pattern(&path, &pattern2, &cdir2) {
                    let mut fmap = files2.lock().unwrap();
                    match ev.kind {
                        EventKind::Create(_) | EventKind::Modify(_) => {
                            if let std::collections::hash_map::Entry::Vacant(e) =
                                fmap.entry(fname.clone())
                            {
                                if let Ok(file) = open_file_shared(&path) {
                                    let (stx, srx) = channel();
                                    e.insert(stx);
                                    let tx = tx2.clone();
                                    let srx = Arc::new(Mutex::new(srx));
                                    thread::spawn(move || follow_file(file, srx, true, fname, tx));
                                }
                            } else if let Some(stx) = fmap.get(&fname) {
                                let _ = stx.send(Signal::Check);
                            }
                        }
                        EventKind::Remove(_) => {
                            if let Some(stx) = fmap.remove(&fname) {
                                let _ = stx.send(Signal::Stop);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    let mut clipboard = Clipboard::new().ok();
    let res = run_app(&mut terminal, &mut app, rx, parser, &mut clipboard);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    for (_, stx) in files.lock().unwrap().drain() {
        let _ = stx.send(Signal::Stop);
    }

    res?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_formats() {
        let c = config::AppConfig::default();
        assert!(c.formats.contains_key("indexserve"));
        assert!(c.formats.contains_key("common"));
        assert!(c.formats.contains_key("json"));
        assert!(c.formats.contains_key("plain"));
    }
}
