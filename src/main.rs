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

fn follow_file(mut file: File, rx: Arc<Mutex<Receiver<()>>>) {
    let mut position = file.seek(SeekFrom::End(0)).unwrap();
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
                print!("{}", buffer);
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
    // Get folder path from the pattern_str
    let current_dir = env::current_dir().unwrap();
    let folder = PathBuf::from(&pattern_str).parent().map(PathBuf::from).unwrap_or_else(|| current_dir);

    let (tx, rx) = channel();

    let (file_tx, file_rx) = channel();
    let file_rx = Arc::new(Mutex::new(file_rx));

    // Start the file watcher in non-recursive mode for the current directory
    let watcher_config = Config::default()
                                    .with_poll_interval(Duration::from_secs(2))
                                    .with_compare_contents(true);
    let mut watcher: RecommendedWatcher = Watcher::new(tx, watcher_config)?;
    watcher.watch(folder.as_path(), RecursiveMode::NonRecursive)?;

    let mut open_files = HashMap::new();

    loop {
        match rx.recv() {
            Ok(Err(e)) => eprintln!("watch error: {:?}", e),
            Ok(Ok(event)) => match event {
                Event { kind: EventKind::Modify(_), paths, .. } | Event { kind: EventKind::Create(_), paths, .. } => {
                    for path in paths {
                        if pattern.matches_path(&path) && !open_files.contains_key(&path) {
                            let file = File::open(&path).unwrap();
                            // Add the file to the open files
                            open_files.insert(path.clone(), file_tx.clone());
                            // Clone the channel so the thread can signal when it should try reading
                            let file_rx_clone = Arc::clone(&file_rx);
                            // Start following the file in a new thread
                            thread::spawn(move || follow_file(file, file_rx_clone));
                        }
                    }
                }
                _ => {}
            },
            Err(e) => eprintln!("watch error: {:?}", e),
        }

        // We need to signal all open files that they should check for new content
        for tx in open_files.values() {
            let _ = tx.send(());
        }
    }
}
