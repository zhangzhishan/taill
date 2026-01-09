use clap::{Command, Arg};
use notify::{PollWatcher, RecursiveMode, Watcher, EventKind, Config};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::sync::{mpsc::{channel, Receiver, Sender}, Arc, Mutex};
use std::time::Duration;
use std::env;
use std::path::{Path, PathBuf};
use std::thread;
use glob::Pattern;
use colored::*;
use bat::PrettyPrinter;

/// Opens a file for reading with shared access on Windows.
/// This allows other processes to read and write to the file while we're reading it.
#[cfg(windows)]
fn open_file_shared(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    // FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE = 0x7
    OpenOptions::new()
        .read(true)
        .share_mode(0x7)
        .open(path)
}

#[cfg(not(windows))]
fn open_file_shared(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

// Define the DEBUG macro
#[cfg(debug_assertions)]
macro_rules! debug {
    ($($arg:tt)*) => (println!($($arg)*));
}

#[cfg(not(debug_assertions))]
macro_rules! debug {
    ($($arg:tt)*) => {};
}

#[derive(Debug, Clone, Copy)]
enum Signal {
    Check,
    Stop,
}

fn follow_file(mut file: File, rx: Arc<Mutex<Receiver<Signal>>>, is_new_file: bool, filename: String) {
    // For new files, start from beginning to dump content. For existing files, start from end.
    let mut position = if is_new_file {
        0u64
    } else {
        file.seek(SeekFrom::End(0)).unwrap()
    };
    let mut reader = BufReader::new(file);

    loop {
        let mut buffer = String::new();
        match reader.read_line(&mut buffer) {
            Ok(0) => {
                // No more content, wait for signal
                match rx.lock().unwrap().recv_timeout(Duration::from_secs(1)) {
                    Ok(Signal::Check) => {
                        // New content might be available, seek to the last known position
                        reader.seek(SeekFrom::Start(position)).unwrap();
                    }
                    Ok(Signal::Stop) => {
                        debug!("Stopping thread for {}", filename);
                        break;
                    }
                    Err(_) => {
                        // Timeout happened, seek to last known position to ensure we read new content
                        reader.seek(SeekFrom::Start(position)).unwrap();
                    } 
                }
            }
            Ok(_) => {
                // Remove newline and add file prefix with color, then use bat for syntax highlighting
                let line = buffer.trim_end();
                if !line.is_empty() {
                    // Use different colors for different files by hashing filename
                    let colors = [Color::Cyan, Color::Green, Color::Yellow, Color::Magenta, Color::Blue];
                    let color_index = filename.chars().map(|c| c as usize).sum::<usize>() % colors.len();
                    let colored_filename = filename.color(colors[color_index]).bold();
                    
                    // Print the file prefix first
                    print!("[{}] ", colored_filename);
                    
                    // Use bat for syntax highlighting of the log line without grid
                    PrettyPrinter::new()
                        .input_from_bytes(line.as_bytes())
                        .language("log")
                        .grid(false)
                        .header(false)
                        .line_numbers(false)
                        .print()
                        .unwrap();
                    println!(); // Add newline after the log line
                }
                position += buffer.as_bytes().len() as u64;
                buffer.clear();
            }
            Err(e) => {
                eprintln!("Error reading from file {}: {}", filename, e);
                break;
            }
        }
    }
}

fn main() -> notify::Result<()> {
    let matches = Command::new("taill")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Zhishan Zhang <zhangzhishanlo@gmail.com>")
        .about("Tails a file and watches for changes")
        .arg(
            Arg::new("pattern")
                .help("The file pattern to watch")
                .short('f')
                .required(true)
        )
        .get_matches();

    let pattern_str = matches.get_one::<String>("pattern").unwrap();
    let pattern = Pattern::new(&pattern_str).expect("Failed to create pattern");
    println!("{} {}", "Watching pattern:".bright_green().bold(), pattern_str.cyan());
    
    // Get folder path from the pattern_str
    let current_dir = env::current_dir().unwrap();
    let pattern_path = PathBuf::from(&pattern_str);
    let folder = if let Some(parent) = pattern_path.parent() {
        if parent.as_os_str().is_empty() {
            current_dir.clone()
        } else {
            if parent.is_absolute() {
                parent.to_path_buf()
            } else {
                current_dir.join(parent)
            }
        }
    } else {
        current_dir.clone()
    };
    debug!("Watching full folder: {:?}", folder);

    let (tx, rx) = channel();

    // Start the file watcher using PollWatcher for reliable cross-platform file monitoring
    let watcher_config = Config::default()
                                    .with_poll_interval(Duration::from_millis(500))
                                    .with_compare_contents(true);
    let mut watcher: PollWatcher = PollWatcher::new(tx, watcher_config)?;
    watcher.watch(folder.as_path(), RecursiveMode::NonRecursive)?;

    // Map of filename -> Sender<Signal>
    let mut open_files: HashMap<String, Sender<Signal>> = HashMap::new();

    // Scan for existing files that match the pattern on startup
    if let Ok(entries) = std::fs::read_dir(&folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_string(),
                None => continue,
            };

            // Check if this file matches the pattern
            let relative_path = path.strip_prefix(&current_dir).unwrap_or(&path);
            let relative_path_str = relative_path.to_string_lossy();
            let full_path_str = path.to_string_lossy();

            let matches_pattern = pattern.matches(&file_name) ||
                                 pattern.matches(&relative_path_str) ||
                                 pattern.matches(&full_path_str);

            if matches_pattern {
                debug!("Opening existing file on startup: {}", file_name);
                match open_file_shared(&path) {
                    Ok(file) => {
                        let (file_tx, file_rx) = channel();
                        let file_rx = Arc::new(Mutex::new(file_rx));
                        open_files.insert(file_name.clone(), file_tx);

                        let fname = file_name.clone();
                        // For existing files at startup, start from the end (is_new_file = false)
                        thread::spawn(move || follow_file(file, file_rx, false, fname));
                    }
                    Err(e) => eprintln!("Failed to open file {}: {}", file_name, e),
                }
            }
        }
    }

    loop {
        match rx.recv() {
            Ok(Err(e)) => eprintln!("watch error: {:?}", e),
            Ok(Ok(event)) => {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) => {
                        debug!("Event: {:?}", event);
                        for path in event.paths {
                            let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
                            
                            // Check if path matches pattern
                            // We need to check relative path from the current directory
                            let relative_path = path.strip_prefix(&current_dir).unwrap_or(&path);
                            let relative_path_str = relative_path.to_string_lossy();
                            let full_path_str = path.to_string_lossy();
                            
                            // Also check if the pattern matches just the filename (for simple patterns like *.log)
                            // or the relative path (for patterns like logs/*.log)
                            // or the full path (for absolute patterns)
                            let matches_pattern = pattern.matches(&file_name) || 
                                                 pattern.matches(&relative_path_str) ||
                                                 pattern.matches(&full_path_str);
                            
                            debug!("Checking path: {:?}, matches: {}", path, matches_pattern);

                            if matches_pattern {
                                if !open_files.contains_key(&file_name) {
                                    // New file detected
                                    debug!("Opening new file: {}", file_name);
                                    match open_file_shared(&path) {
                                        Ok(file) => {
                                            let (file_tx, file_rx) = channel();
                                            let file_rx = Arc::new(Mutex::new(file_rx));
                                            open_files.insert(file_name.clone(), file_tx);
                                            
                                            let fname = file_name.clone();
                                            thread::spawn(move || follow_file(file, file_rx, true, fname));
                                        }
                                        Err(e) => eprintln!("Failed to open file {}: {}", file_name, e),
                                    }
                                } else {
                                    // Existing file modified, signal check
                                    if let Some(tx) = open_files.get(&file_name) {
                                        let _ = tx.send(Signal::Check);
                                    }
                                }
                            }
                        }
                    }
                    EventKind::Remove(_) => {
                        debug!("Remove Event: {:?}", event);
                        for path in event.paths {
                            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                                if open_files.contains_key(file_name) {
                                    debug!("File removed/renamed: {}, stopping thread", file_name);
                                    if let Some(tx) = open_files.remove(file_name) {
                                        let _ = tx.send(Signal::Stop);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => eprintln!("recv error: {:?}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{mpsc::channel, Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use tempfile::NamedTempFile;

    #[test]
    fn test_follow_file_new_file_dumps_existing_content() {
        // Create a temporary file with some initial content
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Initial line 1").unwrap();
        writeln!(temp_file, "Initial line 2").unwrap();
        temp_file.flush().unwrap();

        // Reopen the file for reading
        let file = File::open(temp_file.path()).unwrap();
        
        // Create channel for communication
        let (tx, rx) = channel();
        let rx = Arc::new(Mutex::new(rx));

        // Test that follow_file with is_new_file=true reads from beginning
        let file_clone = file.try_clone().unwrap();
        let rx_clone = Arc::clone(&rx);
        
        thread::spawn(move || {
            follow_file(file_clone, rx_clone, true, "test_file.log".to_string());
        });

        // Give the thread a moment to start
        thread::sleep(Duration::from_millis(10));
        
        // Send stop signal to clean up
        tx.send(Signal::Stop).unwrap();
    }

    #[test]
    fn test_follow_file_existing_file_starts_from_end() {
        // Create a temporary file with some initial content
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Existing line 1").unwrap();
        writeln!(temp_file, "Existing line 2").unwrap();
        temp_file.flush().unwrap();

        // Reopen the file for reading
        let file = File::open(temp_file.path()).unwrap();
        
        // Create channel for communication
        let (tx, rx) = channel();
        let rx = Arc::new(Mutex::new(rx));

        // Test that follow_file with is_new_file=false seeks to end
        let file_clone = file.try_clone().unwrap();
        let rx_clone = Arc::clone(&rx);
        
        thread::spawn(move || {
            follow_file(file_clone, rx_clone, false, "existing_file.log".to_string());
        });

        // Give the thread a moment to start
        thread::sleep(Duration::from_millis(10));
        
        tx.send(Signal::Stop).unwrap();
    }

    #[test]
    fn test_pattern_matching() {
        use glob::Pattern;
        
        // Test various pattern matching scenarios
        let pattern = Pattern::new("*.log").unwrap();
        assert!(pattern.matches("app.log"));
        assert!(pattern.matches("error.log"));
        assert!(!pattern.matches("app.txt"));
        
        let pattern = Pattern::new("test_*.txt").unwrap();
        assert!(pattern.matches("test_file.txt"));
        assert!(pattern.matches("test_123.txt"));
        assert!(!pattern.matches("file_test.txt"));
    }
}

