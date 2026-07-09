//! Aseprite import: the standard JSON (array) + packed PNG export becomes a [`SpriteSheet`]
//! plus one looping [`AnimationClip`] per tag.
//!
//! Export from Aseprite with:
//! `aseprite -b player.ase --sheet player.png --data player.json --format json-array --list-tags`

use bevy_ecs::prelude::Res;
use bevy_ecs::system::{ResMut, SystemParam};
use fulcrum_asset::{AssetError, AssetServer, Assets, Handle};
use fulcrum_core::{FulcrumConfig, FxHashMap, Rect, vec2};
use fulcrum_render::texture::load_texture;
use fulcrum_render::{GpuContext, SpriteSheet, Texture};
use serde::Deserialize;

use crate::clip::AnimationClip;

#[derive(Deserialize)]
struct AseFile {
    frames: Vec<AseFrame>,
    meta: AseMeta,
}

#[derive(Deserialize)]
struct AseFrame {
    #[serde(default)]
    filename: String,
    frame: AseRect,
    /// Frame duration in milliseconds.
    duration: u32,
}

#[derive(Deserialize)]
struct AseRect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

#[derive(Deserialize)]
struct AseMeta {
    image: String,
    #[serde(rename = "frameTags", default)]
    frame_tags: Vec<AseTag>,
}

#[derive(Deserialize)]
struct AseTag {
    name: String,
    from: u32,
    to: u32,
    #[serde(default)]
    direction: String,
}

/// Everything an Aseprite file yields: the sheet (frame regions named by filename) and one
/// looping clip per tag.
pub struct AsepriteImport {
    /// Sheet with one region per exported frame.
    pub sheet: Handle<SpriteSheet>,
    /// Clips by tag name.
    pub clips: FxHashMap<String, Handle<AnimationClip>>,
}

/// Expand a tag's frame range by direction: `forward`, `reverse`, or `pingpong`
/// (`0 1 2 3` -> `0 1 2 3 2 1`).
fn expand_tag(from: u32, to: u32, direction: &str) -> Vec<u32> {
    let forward: Vec<u32> = (from..=to).collect();
    match direction {
        "reverse" => forward.into_iter().rev().collect(),
        "pingpong" => {
            let mut frames = forward.clone();
            if forward.len() > 2 {
                frames.extend(forward[1..forward.len() - 1].iter().rev());
            }
            frames
        }
        _ => forward,
    }
}

fn ms_to_ticks(ms: u32, tick_rate: u32) -> u32 {
    ((ms as f32 / 1000.0 * tick_rate as f32).round() as u32).max(1)
}

fn parse(path: &str, bytes: &[u8]) -> Result<AseFile, AssetError> {
    serde_json::from_slice(bytes).map_err(|error| AssetError::Decode {
        path: path.to_string(),
        message: format!("not an Aseprite JSON-array export: {error}"),
    })
}

/// One-line Aseprite loading for game systems:
/// `let player = aseprite.load("player.json")?;`
#[derive(SystemParam)]
pub struct AsepriteLoader<'w> {
    server: Res<'w, AssetServer>,
    textures: ResMut<'w, Assets<Texture>>,
    sheets: ResMut<'w, Assets<SpriteSheet>>,
    clips: ResMut<'w, Assets<AnimationClip>>,
    gpu: Res<'w, GpuContext>,
    config: Res<'w, FulcrumConfig>,
}

impl AsepriteLoader<'_> {
    /// Read access to the sheet storage (e.g. to look up named regions after loading).
    pub fn sheets(&self) -> &Assets<SpriteSheet> {
        &self.sheets
    }

    /// Load an Aseprite JSON export (and its packed PNG, resolved relative to the JSON's
    /// directory). Frame durations convert from milliseconds to simulation ticks.
    pub fn load(&mut self, json_path: &str) -> Result<AsepriteImport, AssetError> {
        let bytes = self.server.read_bytes(json_path)?;
        let file = parse(json_path, &bytes)?;

        // The packed image lives next to the JSON.
        let dir = std::path::Path::new(json_path)
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""));
        let image_path = dir.join(&file.meta.image);
        let texture = load_texture(
            &self.server,
            &mut self.textures,
            &self.gpu,
            &image_path.to_string_lossy(),
        );

        let mut sheet = SpriteSheet {
            texture,
            regions: Vec::with_capacity(file.frames.len()),
            names: FxHashMap::default(),
        };
        for (index, frame) in file.frames.iter().enumerate() {
            sheet.regions.push(Rect::from_min_size(
                vec2(frame.frame.x, frame.frame.y),
                vec2(frame.frame.w, frame.frame.h),
            ));
            if !frame.filename.is_empty() {
                sheet.names.insert(frame.filename.clone(), index as u32);
            }
        }
        let sheet = self.sheets.insert_with_path(json_path, sheet);

        let tick_rate = self.config.tick_rate;
        let mut clips = FxHashMap::default();
        for tag in &file.meta.frame_tags {
            if tag.to as usize >= file.frames.len() || tag.from > tag.to {
                return Err(AssetError::Decode {
                    path: json_path.to_string(),
                    message: format!(
                        "tag `{}` range {}..={} is out of bounds ({} frames)",
                        tag.name,
                        tag.from,
                        tag.to,
                        file.frames.len()
                    ),
                });
            }
            let frames = expand_tag(tag.from, tag.to, &tag.direction);
            let frame_ticks = frames
                .iter()
                .map(|&f| ms_to_ticks(file.frames[f as usize].duration, tick_rate))
                .collect();
            let clip = AnimationClip {
                sheet,
                frames,
                frame_ticks,
                looping: true,
            };
            clips.insert(tag.name.clone(), self.clips.insert(clip));
        }

        Ok(AsepriteImport { sheet, clips })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
        "frames": [
            { "filename": "hero 0", "frame": {"x":0,"y":0,"w":16,"h":16}, "duration": 100 },
            { "filename": "hero 1", "frame": {"x":16,"y":0,"w":16,"h":16}, "duration": 100 },
            { "filename": "hero 2", "frame": {"x":32,"y":0,"w":16,"h":16}, "duration": 50 },
            { "filename": "hero 3", "frame": {"x":0,"y":16,"w":16,"h":16}, "duration": 200 }
        ],
        "meta": {
            "image": "hero.png",
            "frameTags": [
                { "name": "idle", "from": 0, "to": 1, "direction": "forward" },
                { "name": "spin", "from": 1, "to": 3, "direction": "pingpong" }
            ]
        }
    }"#;

    #[test]
    fn parses_frames_and_tags() {
        let file = parse("hero.json", FIXTURE.as_bytes()).unwrap();
        assert_eq!(file.frames.len(), 4);
        assert_eq!(file.meta.image, "hero.png");
        assert_eq!(file.meta.frame_tags.len(), 2);
        assert_eq!(file.frames[2].duration, 50);
        assert_eq!(file.frames[3].frame.y, 16.0);
    }

    #[test]
    fn tag_directions_expand_correctly() {
        assert_eq!(expand_tag(0, 3, "forward"), vec![0, 1, 2, 3]);
        assert_eq!(expand_tag(0, 3, "reverse"), vec![3, 2, 1, 0]);
        assert_eq!(expand_tag(1, 3, "pingpong"), vec![1, 2, 3, 2]);
        assert_eq!(expand_tag(0, 1, "pingpong"), vec![0, 1], "short pingpong");
        assert_eq!(expand_tag(2, 2, "forward"), vec![2], "single frame");
    }

    #[test]
    fn durations_convert_ms_to_ticks() {
        assert_eq!(ms_to_ticks(100, 60), 6);
        assert_eq!(ms_to_ticks(50, 60), 3);
        assert_eq!(ms_to_ticks(200, 60), 12);
        assert_eq!(ms_to_ticks(1, 60), 1, "minimum one tick");
    }

    #[test]
    fn malformed_json_is_a_descriptive_error_not_a_panic() {
        let Err(error) = parse("bad.json", b"{ not json") else {
            panic!("malformed json parsed");
        };
        let message = error.to_string();
        assert!(message.contains("bad.json"));
        assert!(message.contains("Aseprite"));
    }
}
