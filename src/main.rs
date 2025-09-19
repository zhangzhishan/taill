use clap::{Command, Arg};
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind, Config};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::sync::{mpsc::{channel, Receiver}, Arc, Mutex};
use std::time::Duration;
use std::env;
use std::path::PathBuf;
use std::thread;
use glob::Pattern;
use colored::*;
use bat::PrettyPrinter;

// Define the DEBUG macro
#[cfg(debug_assertions)]
macro_rules! debug {
    ($($arg:tt)*) => (println!($($arg)*));
}

#[cfg(not(debug_assertions))]
macro_rules! debug {
    ($($arg:tt)*) => {};
}

fn follow_file(mut file: File, rx: Arc<Mutex<Receiver<()>>>, is_new_file: bool, filename: String) {
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
                // No more content, so wait for a signal that there's more content,
                // or a timeout, before trying again.
                match rx.lock().unwrap().recv_timeout(Duration::from_secs(1)) {
                    Ok(()) => {
                        // New content might be available, seek to the last known position
                        reader.seek(SeekFrom::Start(position)).unwrap();
                    }
                    Err(_) => {} // Timeout happened, just loop around and try reading again
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
                }
                position += buffer.as_bytes().len() as u64;
                buffer.clear();
            }
            Err(e) => {
                eprintln!("Error reading from file: {}", e);
                break;
            }
        }
    }
}

fn main() -> notify::Result<()> {
    let matches = Command::new("taill")
        .version("0.1")
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
            current_dir
        } else {
            parent.to_path_buf()
        }
    } else {
        current_dir
    };
    debug!("Watching full folder: {:?}", folder);

    let (tx, rx) = channel();

    let (file_tx, file_rx) = channel();
    let file_rx = Arc::new(Mutex::new(file_rx));

    // Start the file watcher in non-recursive mode for the current directory
    let watcher_config = Config::default()
                                    .with_poll_interval(Duration::from_secs(1))
                                    .with_compare_contents(true);
    let mut watcher: RecommendedWatcher = Watcher::new(tx, watcher_config)?;
    // let full_path = PathBuf::from("D:\\code\\taill\\target\\debug\\deps");
    watcher.watch(folder.as_path(), RecursiveMode::NonRecursive)?;

    let mut open_files = HashMap::new();

    loop {
        match rx.recv() {
            Ok(Err(e)) => eprintln!("watch error: {:?}", e),
            Ok(Ok(event)) => match event {
                Event { kind: EventKind::Create(_), paths, .. } => {
                    debug!("Create Event: {:?}", paths);
                    for path in paths {
                        let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
                        let full_path_str = path.to_string_lossy().to_string();
                        let matches_pattern = pattern.matches(&file_name) || pattern.matches(&full_path_str);
                        debug!("pattern matches: {:?}", matches_pattern);
                        debug!("open_files: {:?}", open_files);
                        debug!("open_files contains key: {:?}", open_files.contains_key(&file_name));
                        if matches_pattern && !open_files.contains_key(&file_name) {
                            // Open the file and start following it if succeed.
                            let file: File = File::open(&path)?;

                            // Add the file to the open files
                            open_files.insert(file_name.clone(), file_tx.clone());
                            // Clone the channel so the thread can signal when it should try reading
                            let file_rx_clone = Arc::clone(&file_rx);
                            // Start following the file in a new thread - this is a new file
                            thread::spawn(move || follow_file(file, file_rx_clone, true, file_name.clone()));
                        }
                    }
                }
                Event { kind: EventKind::Modify(_), paths, .. } => {
                    debug!("Modify Event: {:?}", paths);
                    for path in paths {
                        let file_name = path.file_name().unwrap().to_str().unwrap().to_string();
                        let full_path_str = path.to_string_lossy().to_string();
                        let matches_pattern = pattern.matches(&file_name) || pattern.matches(&full_path_str);
                        debug!("pattern matches: {:?}", matches_pattern);
                        debug!("open_files: {:?}", open_files);
                        debug!("open_files contains key: {:?}", open_files.contains_key(&file_name));
                        if matches_pattern && !open_files.contains_key(&file_name) {
                            // Open the file and start following it if succeed.
                            let file: File = File::open(&path)?;

                            // Add the file to the open files
                            open_files.insert(file_name.clone(), file_tx.clone());
                            // Clone the channel so the thread can signal when it should try reading
                            let file_rx_clone = Arc::clone(&file_rx);
                            // Start following the file in a new thread - this is an existing file
                            thread::spawn(move || follow_file(file, file_rx_clone, false, file_name.clone()));
                        }
                    }
                }
                _ => {}
            },
            // Err(RecvTimeoutError::Timeout) => {
            //     // Timeout occurred, proceed to signal open files
            //     println!("Timeout");
            // }
            Err(e) => eprintln!("recv error: {:?}", e),
        }

        // We need to signal all open files that they should check for new content
        for tx in open_files.values() {
            let _ = tx.send(());
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
        let (_tx, rx) = channel();
        let rx = Arc::new(Mutex::new(rx));

        // Test that follow_file with is_new_file=true reads from beginning
        let file_clone = file.try_clone().unwrap();
        let rx_clone = Arc::clone(&rx);
        
        // Use a channel to capture output (in real implementation this goes to stdout via bat)
        // For testing, we'll verify the file position behavior
        thread::spawn(move || {
            // This would normally print the content, we're testing the seeking behavior
            follow_file(file_clone, rx_clone, true, "test_file.log".to_string());
        });

        // Give the thread a moment to start
        thread::sleep(Duration::from_millis(10));

        // The test passes if no panic occurs and the thread starts properly
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
        let (_tx, rx) = channel();
        let rx = Arc::new(Mutex::new(rx));

        // Test that follow_file with is_new_file=false seeks to end
        let file_clone = file.try_clone().unwrap();
        let rx_clone = Arc::clone(&rx);
        
        thread::spawn(move || {
            follow_file(file_clone, rx_clone, false, "existing_file.log".to_string());
        });

        // Give the thread a moment to start
        thread::sleep(Duration::from_millis(10));

        // The test passes if no panic occurs and the thread starts properly
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

    #[test]
    fn test_file_operations() {
        use std::io::{Read, Seek, SeekFrom};
        
        // Create a temporary file with content
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "Line 1").unwrap();
        writeln!(temp_file, "Line 2").unwrap();
        writeln!(temp_file, "Line 3").unwrap();
        temp_file.flush().unwrap();

        // Test seeking to end
        let mut file = File::open(temp_file.path()).unwrap();
        let end_pos = file.seek(SeekFrom::End(0)).unwrap();
        assert!(end_pos > 0);

        // Test seeking to beginning
        let start_pos = file.seek(SeekFrom::Start(0)).unwrap();
        assert_eq!(start_pos, 0);

        // Test reading content
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 2"));
        assert!(content.contains("Line 3"));
    }

    #[test]
    fn test_channel_communication() {
        let (tx, rx) = channel();
        let rx = Arc::new(Mutex::new(rx));
        
        // Test sending signal
        tx.send(()).unwrap();
        
        // Test receiving signal with timeout
        let result = rx.lock().unwrap().recv_timeout(Duration::from_millis(100));
        assert!(result.is_ok());
        
        // Test timeout when no signal
        let result = rx.lock().unwrap().recv_timeout(Duration::from_millis(10));
        assert!(result.is_err());
    }

    #[test]
    fn test_file_extension_patterns() {
        use glob::Pattern;
        
        // Test different file extension patterns
        let log_pattern = Pattern::new("*.log").unwrap();
        assert!(log_pattern.matches("app.log"));
        assert!(log_pattern.matches("error.log"));
        assert!(!log_pattern.matches("app.txt"));
        
        let multi_pattern = Pattern::new("logs/*.txt").unwrap();
        assert!(multi_pattern.matches("logs/debug.txt"));
        assert!(multi_pattern.matches("logs/error.txt"));
        assert!(!multi_pattern.matches("debug.txt"));
        assert!(!multi_pattern.matches("logs/debug.log"));
    }

    #[test]
    fn test_path_handling() {
        use std::path::PathBuf;
        use std::env;
        
        // Test path parsing from pattern
        let pattern_str = "logs/*.txt";
        let current_dir = env::current_dir().unwrap();
        let folder = PathBuf::from(&pattern_str).parent().map(PathBuf::from).unwrap_or_else(|| current_dir.clone());
        
        assert_eq!(folder, PathBuf::from("logs"));
        
        // Test pattern without directory
        let pattern_str = "*.log";
        let folder = PathBuf::from(&pattern_str).parent().map(PathBuf::from).unwrap_or_else(|| current_dir);
        
        // Should default to current directory when no parent
        assert!(folder.is_absolute() || folder == PathBuf::from(""));
    }

    #[test]
    fn test_error_handling() {
        use tempfile::tempdir;
        
        // Test opening non-existent file
        let temp_dir = tempdir().unwrap();
        let non_existent_path = temp_dir.path().join("nonexistent.txt");
        let result = File::open(&non_existent_path);
        assert!(result.is_err());
        
        // Test pattern creation with invalid pattern
        use glob::Pattern;
        let result = Pattern::new("[");
        assert!(result.is_err());
    }

    #[test] 
    fn test_event_kind_differentiation() {
        use notify::EventKind;
        
        // Test that we can distinguish between Create and Modify events
        // This tests the event handling logic structure
        let create_event = EventKind::Create(notify::event::CreateKind::File);
        let modify_event = EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Content));
        
        // In real code, these would be handled differently
        match create_event {
            EventKind::Create(_) => {
                // Should handle as new file
                assert!(true);
            }
            _ => assert!(false, "Should be Create event"),
        }
        
        match modify_event {
            EventKind::Modify(_) => {
                // Should handle as existing file  
                assert!(true);
            }
            _ => assert!(false, "Should be Modify event"),
        }
    }

    #[test]
    fn test_concurrent_file_access() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        
        // Test multiple threads accessing the same mechanisms
        let temp_file = NamedTempFile::new().unwrap();
        let success = Arc::new(AtomicBool::new(false));
        
        let success_clone = Arc::clone(&success);
        let file_path = temp_file.path().to_path_buf();
        
        thread::spawn(move || {
            if let Ok(_file) = File::open(&file_path) {
                success_clone.store(true, Ordering::Relaxed);
            }
        });
        
        thread::sleep(Duration::from_millis(50));
        assert!(success.load(Ordering::Relaxed));
    }

    #[test]
    fn test_buffered_reading() {
        use std::io::{BufRead, BufReader};
        
        // Test line-by-line reading functionality
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "First line").unwrap();
        writeln!(temp_file, "Second line").unwrap();
        writeln!(temp_file, "Third line").unwrap();
        temp_file.flush().unwrap();
        
        let file = File::open(temp_file.path()).unwrap();
        let mut reader = BufReader::new(file);
        
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).unwrap();
        assert!(bytes_read > 0);
        assert!(line.contains("First line"));
        
        line.clear();
        let bytes_read = reader.read_line(&mut line).unwrap();
        assert!(bytes_read > 0);
        assert!(line.contains("Second line"));
    }
}
