//! [`AnimationPlayer`]: plays an [`AnimationClip`] on an entity's `Sprite`.

use bevy_ecs::prelude::{Component, Query, Res};
use fulcrum_asset::{Assets, Handle};
use fulcrum_render::{Sprite, SpriteRegion};

use crate::clip::AnimationClip;

/// Plays a clip by writing the current frame into the entity's [`Sprite::region`] every tick.
/// The player owns the sprite's region while attached — don't set it manually.
#[derive(Component, Clone)]
pub struct AnimationPlayer {
    /// The clip being played.
    pub clip: Handle<AnimationClip>,
    /// Whether playback advances (a paused player still shows its current frame).
    pub playing: bool,
    /// Ticks spent on the current frame so far.
    pub tick_in_frame: u32,
    /// Index into the clip's frame list.
    pub frame_index: usize,
    finished: bool,
}

impl AnimationPlayer {
    /// Start playing `clip` from its first frame.
    pub fn play(clip: Handle<AnimationClip>) -> Self {
        Self {
            clip,
            playing: true,
            tick_in_frame: 0,
            frame_index: 0,
            finished: false,
        }
    }

    /// Switch to `clip` from the start — a no-op if it's already the active clip, so calling
    /// this every tick doesn't stutter.
    pub fn restart(&mut self, clip: Handle<AnimationClip>) {
        if self.clip != clip {
            *self = Self::play(clip);
        }
    }

    /// Has a non-looping clip reached the end of its last frame?
    pub fn finished(&self) -> bool {
        self.finished
    }
}

/// `FixedUpdate` system: advance every player one tick and write the frame to the sprite.
pub(crate) fn advance_animations(
    mut players: Query<(&mut AnimationPlayer, &mut Sprite)>,
    clips: Res<Assets<AnimationClip>>,
) {
    for (mut player, mut sprite) in &mut players {
        let Some(clip) = clips.get(player.clip) else {
            continue;
        };
        if clip.frames.is_empty() {
            continue;
        }
        if player.playing {
            player.tick_in_frame += 1;
            let duration = clip
                .frame_ticks
                .get(player.frame_index)
                .copied()
                .unwrap_or(1)
                .max(1);
            if player.tick_in_frame >= duration {
                player.tick_in_frame = 0;
                if player.frame_index + 1 < clip.frames.len() {
                    player.frame_index += 1;
                } else if clip.looping {
                    player.frame_index = 0;
                } else {
                    player.playing = false;
                    player.finished = true;
                }
            }
        }
        // Clamp guards against clips shrinking under a live player (hot reload, phase 3).
        let frame = player.frame_index.min(clip.frames.len() - 1);
        sprite.region = Some(SpriteRegion {
            sheet: clip.sheet,
            index: clip.frames[frame],
        });
    }
}
