//! A frame of the video, drawn next to the fields while a time is chosen.
//!
//! One worker thread turns "the frame at T" into something the terminal can
//! show. Requests are "latest wins": while an arrow key is held, the frames
//! in between are never fetched.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use ratatui::layout::Size;
use ratatui_image::Resize;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;

use crate::app::Event;

const CACHE: usize = 48;

/// Milliseconds into the file, and the size in cells the frame is drawn at.
pub type Key = (i64, u16, u16);

struct Request {
    path: PathBuf,
    key: Key,
}

type Slot = Arc<(Mutex<Option<Request>>, Condvar)>;

pub struct Preview {
    slot: Slot,
    path: PathBuf,
    cache: HashMap<Key, Protocol>,
    order: VecDeque<Key>,
    /// The frame on screen; it stays until the one asked for arrives.
    shown: Option<Key>,
}

/// One frame as PNG bytes, scaled down to `width` pixels.
fn grab(path: &Path, t: f64, width: u32) -> Option<Vec<u8>> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-ss", &format!("{t:.3}"), "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf", &format!("scale={width}:-2")])
        .args(["-f", "image2pipe", "-vcodec", "png", "-compression_level", "1", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    (out.status.success() && !out.stdout.is_empty()).then_some(out.stdout)
}

fn render(picker: &Picker, path: &Path, key: Key) -> Option<Protocol> {
    let (ms, cols, rows) = key;
    // Enough pixels for the cells it will fill, and no more: the frame is
    // decoded, scaled and encoded on every move.
    let width = (cols as u32 * picker.font_size().width as u32).clamp(160, 960);
    let png = grab(path, ms as f64 / 1000.0, width)?;
    let image = image::load_from_memory(&png).ok()?;
    picker.new_protocol(image, Size::new(cols, rows), Resize::Fit(None)).ok()
}

impl Preview {
    pub fn new(picker: Picker, tx: Sender<Event>) -> Self {
        let slot: Slot = Arc::new((Mutex::new(None), Condvar::new()));
        let worker_slot = slot.clone();
        thread::spawn(move || {
            loop {
                let request = {
                    let (lock, ready) = &*worker_slot;
                    let mut guard = lock.lock().unwrap();
                    loop {
                        match guard.take() {
                            Some(r) => break r,
                            None => guard = ready.wait(guard).unwrap(),
                        }
                    }
                };
                let frame = render(&picker, &request.path, request.key);
                if tx.send(Event::Frame { path: request.path, key: request.key, frame }).is_err() {
                    break;
                }
            }
        });
        Preview { slot, path: PathBuf::new(), cache: HashMap::new(), order: VecDeque::new(), shown: None }
    }

    /// Asks for the frame at `t`; a cached one is shown at once.
    pub fn want(&mut self, path: &Path, t: f64, size: (u16, u16)) {
        if self.path != path {
            self.path = path.to_path_buf();
            self.cache.clear();
            self.order.clear();
            self.shown = None;
        }
        let key = ((t * 1000.0).round() as i64, size.0, size.1);
        if self.cache.contains_key(&key) {
            self.shown = Some(key);
            return;
        }
        let (lock, ready) = &*self.slot;
        *lock.lock().unwrap() = Some(Request { path: path.to_path_buf(), key });
        ready.notify_one();
    }

    pub fn accept(&mut self, path: &Path, key: Key, frame: Protocol) {
        if path != self.path {
            return;
        }
        if self.cache.insert(key, frame).is_none() {
            self.order.push_back(key);
        }
        while self.order.len() > CACHE {
            if let Some(old) = self.order.pop_front() {
                self.cache.remove(&old);
            }
        }
        self.shown = Some(key);
    }

    pub fn frame(&self) -> Option<(&Protocol, f64)> {
        let key = self.shown?;
        self.cache.get(&key).map(|p| (p, key.0 as f64 / 1000.0))
    }
}

/// The size of the preview in cells for a terminal of `term` cells and a
/// video of the given aspect ratio. `None` when there is no room for one.
pub fn size(term: (u16, u16), aspect: f64) -> Option<(u16, u16)> {
    let (w, h) = (term.0.saturating_sub(4), term.1);
    // Header, a caption under the frame, the note, the command, the ruler
    // and the keys take seventeen rows.
    let rows = h.saturating_sub(17).min(12);
    if rows < 5 || aspect <= 0.0 {
        return None;
    }
    // A cell is about twice as tall as it is wide.
    let cols = ((rows as f64 * 2.0 * aspect).round() as u16).min(w / 2);
    (cols >= 12).then_some((cols, rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preview_grows_with_the_window_and_keeps_room_for_the_form() {
        assert_eq!(size((80, 24), 16.0 / 9.0), Some((25, 7)));
        assert_eq!(size((120, 40), 16.0 / 9.0), Some((43, 12)));
        assert_eq!(size((80, 20), 16.0 / 9.0), None);
        // A wide video is capped at half the width.
        assert_eq!(size((80, 30), 4.0), Some((38, 12)));
    }
}
