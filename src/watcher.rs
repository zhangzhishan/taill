use glob::Pattern;
use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Seek, SeekFrom},
    path::Path,
    sync::{mpsc::{Receiver, Sender}, Arc, Mutex},
    time::Duration,
};

#[derive(Clone, Copy)]
pub enum Signal {
    Check,
    Stop,
}

pub fn follow_file(
    mut file: File,
    rx: Arc<Mutex<Receiver<Signal>>>,
    is_new: bool,
    filename: String,
    tx: Sender<String>,
) {
    let mut pos = if is_new { 0 } else { file.seek(SeekFrom::End(0)).unwrap_or(0) };
    let mut reader = BufReader::new(file);
    let mut buf = String::new();

    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => match rx.lock().unwrap().recv_timeout(Duration::from_millis(100)) {
                Ok(Signal::Stop) => break,
                _ => { let _ = reader.seek(SeekFrom::Start(pos)); }
            },
            Ok(n) => {
                let line = buf.trim_end();
                if !line.is_empty() {
                    let _ = tx.send(format!("\x00{}\x00{}", filename, line));
                }
                pos += n as u64;
            }
            Err(_) => break,
        }
    }
}

pub fn matches_pattern(path: &Path, pattern: &Pattern, current_dir: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let rel = path.strip_prefix(current_dir).unwrap_or(path);
    if pattern.matches(file_name) || pattern.matches(&rel.to_string_lossy()) || pattern.matches(&path.to_string_lossy()) {
        Some(file_name.into())
    } else {
        None
    }
}

#[cfg(windows)]
pub fn open_file_shared(path: &Path) -> io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    OpenOptions::new().read(true).share_mode(0x7).open(path)
}

#[cfg(not(windows))]
pub fn open_file_shared(path: &Path) -> io::Result<File> {
    File::open(path)
}
