//! What ffprobe says about a file: the form's fields depend on it.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq)]
pub struct Video {
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub codec: String,
    pub bitrate: Option<u64>,
    pub channels: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MediaInfo {
    pub path: PathBuf,
    pub duration: f64,
    pub size: u64,
    pub video: Option<Video>,
    pub audio: Option<Audio>,
}

#[derive(Deserialize)]
struct Raw {
    format: Option<RawFormat>,
    #[serde(default)]
    streams: Vec<RawStream>,
}

#[derive(Deserialize)]
struct RawFormat {
    duration: Option<String>,
    size: Option<String>,
}

#[derive(Deserialize)]
struct RawStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    bit_rate: Option<String>,
    channels: Option<u32>,
    disposition: Option<RawDisposition>,
}

#[derive(Deserialize)]
struct RawDisposition {
    #[serde(default)]
    attached_pic: u8,
}

fn rate(s: &Option<String>) -> Option<f64> {
    let (n, d) = s.as_deref()?.split_once('/')?;
    let (n, d): (f64, f64) = (n.parse().ok()?, d.parse().ok()?);
    (d > 0.0 && n > 0.0).then(|| n / d)
}

pub fn parse(path: &Path, json: &str) -> Result<MediaInfo> {
    let raw: Raw = serde_json::from_str(json).context("ffprobe printed something unexpected")?;
    let format = raw.format.context("ffprobe found no container in the file")?;
    let mut video = None;
    let mut audio = None;
    for s in &raw.streams {
        // Cover art in an mp3 is a video stream to ffprobe, not to us.
        let cover = s.disposition.as_ref().is_some_and(|d| d.attached_pic == 1);
        match s.codec_type.as_deref() {
            Some("video") if video.is_none() && !cover => {
                video = Some(Video {
                    codec: s.codec_name.clone().unwrap_or_default(),
                    width: s.width.unwrap_or(0),
                    height: s.height.unwrap_or(0),
                    fps: rate(&s.avg_frame_rate)
                        .or_else(|| rate(&s.r_frame_rate))
                        .unwrap_or(25.0),
                });
            }
            Some("audio") if audio.is_none() => {
                audio = Some(Audio {
                    codec: s.codec_name.clone().unwrap_or_default(),
                    bitrate: s.bit_rate.as_deref().and_then(|b| b.parse().ok()),
                    channels: s.channels.unwrap_or(0),
                });
            }
            _ => {}
        }
    }
    if video.is_none() && audio.is_none() {
        bail!("there is no video or audio in {}", path.display());
    }
    Ok(MediaInfo {
        path: path.to_path_buf(),
        duration: format
            .duration
            .as_deref()
            .and_then(|d| d.parse().ok())
            .unwrap_or(0.0),
        size: format.size.as_deref().and_then(|d| d.parse().ok()).unwrap_or(0),
        video,
        audio,
    })
}

pub fn probe(path: &Path) -> Result<MediaInfo> {
    if !path.is_file() {
        bail!("there is no file {}", path.display());
    }
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-show_format", "-show_streams", "-of", "json"])
        .arg(path)
        .output()
        .context("could not start ffprobe")?;
    if !out.status.success() {
        bail!("ffprobe cannot read {}", path.display());
    }
    parse(path, &String::from_utf8_lossy(&out.stdout))
}

/// The last keyframe at or before `t`: where a stream-copy cut really starts.
pub fn keyframe_before(path: &Path, t: f64) -> Option<f64> {
    let from = (t - 30.0).max(0.0);
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-skip_frame", "nokey"])
        .args(["-show_entries", "frame=pts_time", "-of", "csv=p=0"])
        .args(["-read_intervals", &format!("{from:.3}%{:.3}", t + 0.001)])
        .arg(path)
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().trim_end_matches(',').parse::<f64>().ok())
        .filter(|k| *k <= t + 0.001)
        .fold(None, |best: Option<f64>, k| Some(best.map_or(k, |b| b.max(k))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_video_and_audio() {
        let json = r#"{"streams":[
            {"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"25/1","disposition":{"attached_pic":0}},
            {"codec_type":"audio","codec_name":"aac","bit_rate":"128000","channels":2}],
            "format":{"duration":"130.000000","size":"48000000"}}"#;
        let info = parse(Path::new("a.mov"), json).unwrap();
        assert_eq!(info.duration, 130.0);
        assert_eq!(info.size, 48_000_000);
        assert_eq!(info.video.as_ref().unwrap().height, 1080);
        assert_eq!(info.audio.as_ref().unwrap().codec, "aac");
    }

    #[test]
    fn cover_art_is_not_video() {
        let json = r#"{"streams":[
            {"codec_type":"video","codec_name":"mjpeg","width":500,"height":500,"disposition":{"attached_pic":1}},
            {"codec_type":"audio","codec_name":"mp3","channels":2}],
            "format":{"duration":"10","size":"1000"}}"#;
        let info = parse(Path::new("a.mp3"), json).unwrap();
        assert!(info.video.is_none());
        assert!(info.audio.is_some());
    }
}
