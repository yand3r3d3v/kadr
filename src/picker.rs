//! The file list shown when kadr starts without a file.

use std::path::{Path, PathBuf};

use crate::probe::MediaInfo;

const MEDIA: [&str; 19] = [
    "mp4", "mov", "mkv", "webm", "avi", "m4v", "mpg", "mpeg", "ts", "wmv", "flv", "mp3", "m4a",
    "wav", "flac", "ogg", "opus", "aac", "gif",
];

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    /// Filled in from a background thread, file by file.
    pub info: Option<MediaInfo>,
}

#[derive(Debug)]
pub struct Picker {
    pub dir: PathBuf,
    pub entries: Vec<Entry>,
    pub filter: String,
    pub selected: usize,
    /// Bumped on every directory change so late probe results are dropped.
    pub generation: u64,
}

fn is_media(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MEDIA.contains(&e.to_ascii_lowercase().as_str()))
}

impl Picker {
    pub fn new(dir: PathBuf) -> Self {
        let mut p = Picker { dir, entries: vec![], filter: String::new(), selected: 0, generation: 0 };
        p.reload();
        p
    }

    fn join(&self, name: &str) -> PathBuf {
        // Keep paths the way a person would type them: no leading "./".
        if self.dir == Path::new(".") { PathBuf::from(name) } else { self.dir.join(name) }
    }

    pub fn reload(&mut self) {
        self.generation += 1;
        self.filter.clear();
        self.selected = 0;
        let mut dirs = vec![];
        let mut files = vec![];
        if let Ok(read) = std::fs::read_dir(&self.dir) {
            for e in read.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with('.') {
                    continue;
                }
                let path = self.join(&name);
                if path.is_dir() {
                    dirs.push(Entry { name: format!("{name}/"), path, is_dir: true, info: None });
                } else if is_media(&path) {
                    files.push(Entry { name, path, is_dir: false, info: None });
                }
            }
        }
        files.sort_by_key(|e| e.name.to_lowercase());
        dirs.sort_by_key(|e| e.name.to_lowercase());
        self.entries = files;
        self.entries.push(Entry { name: "../".into(), path: self.join(".."), is_dir: true, info: None });
        self.entries.extend(dirs);
    }

    /// Indices of the entries that match the filter.
    pub fn visible(&self) -> Vec<usize> {
        let needle = self.filter.to_lowercase();
        (0..self.entries.len())
            .filter(|i| self.entries[*i].name.to_lowercase().contains(&needle))
            .collect()
    }

    pub fn current(&self) -> Option<&Entry> {
        self.visible().get(self.selected).map(|i| &self.entries[*i])
    }

    pub fn files(&self) -> usize {
        self.entries.iter().filter(|e| !e.is_dir).count()
    }

    pub fn move_by(&mut self, delta: isize) {
        let n = self.visible().len();
        if n > 0 {
            self.selected = (self.selected as isize + delta).clamp(0, n as isize - 1) as usize;
        }
    }

    pub fn enter_dir(&mut self, path: PathBuf) {
        self.dir = path;
        self.reload();
    }
}
