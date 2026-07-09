//! Fulcrum audio: [`Sound`] assets and the [`Audio`] resource, backed by kira.
//!
//! Playback is **cosmetic**: sounds are triggered from simulation systems freely, but nothing
//! about playback feeds back into the simulation, so audio is exempt from the determinism
//! contract. On machines with no audio device the engine runs silently instead of failing.

use std::io::Cursor;
use std::sync::Mutex;

use bevy_ecs::prelude::{Res, ResMut, Resource};
use bevy_ecs::system::SystemParam;
use fulcrum_asset::{AssetError, AssetServer, Assets, Handle};
use fulcrum_core::{Fulcrum, Plugin};
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween};

/// A loaded sound (WAV/OGG/MP3/FLAC), cheap to clone.
pub struct Sound(StaticSoundData);

/// One-shot playback parameters.
#[derive(Clone, Copy, Debug)]
pub struct PlayParams {
    /// Linear volume, `0.0..=1.0` (converted to decibels internally).
    pub volume: f32,
    /// Playback rate multiplier (1.0 = normal; also shifts pitch).
    pub pitch: f32,
    /// Stereo pan, `-1.0` (left) to `1.0` (right).
    pub pan: f32,
}

impl Default for PlayParams {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pitch: 1.0,
            pan: 0.0,
        }
    }
}

fn to_decibels(linear: f32) -> Decibels {
    if linear <= 0.0 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * linear.log10())
    }
}

/// The audio system. `audio.play(&sounds, handle)` and go.
///
/// kira's manager isn't `Sync`, so it lives behind a mutex; contention is zero in practice
/// (audio calls come from the single-threaded simulation).
#[derive(Resource)]
pub struct Audio {
    manager: Option<Mutex<AudioManager<DefaultBackend>>>,
    music: Option<StaticSoundHandle>,
}

impl Audio {
    fn new() -> Self {
        let manager = match AudioManager::new(AudioManagerSettings::default()) {
            Ok(manager) => Some(Mutex::new(manager)),
            Err(error) => {
                log::warn!("no audio device ({error}); running silent");
                None
            }
        };
        Self {
            manager,
            music: None,
        }
    }

    /// Fire-and-forget playback with default parameters.
    pub fn play(&mut self, sounds: &Assets<Sound>, sound: Handle<Sound>) {
        self.play_with(sounds, sound, PlayParams::default());
    }

    /// Fire-and-forget playback with volume/pitch/pan.
    pub fn play_with(&mut self, sounds: &Assets<Sound>, sound: Handle<Sound>, params: PlayParams) {
        let Some(manager) = &self.manager else { return };
        let Some(sound) = sounds.get(sound) else {
            log::error!("Audio::play: unknown sound handle");
            return;
        };
        let data = sound
            .0
            .volume(to_decibels(params.volume))
            .playback_rate(params.pitch as f64)
            .panning(params.pan);
        if let Err(error) = manager.lock().unwrap().play(data) {
            log::error!("failed to play sound: {error}");
        }
    }

    /// Play music in the single music slot, replacing whatever was playing.
    pub fn play_music(&mut self, sounds: &Assets<Sound>, sound: Handle<Sound>, looping: bool) {
        self.stop_music();
        let Some(manager) = &self.manager else { return };
        let Some(sound) = sounds.get(sound) else {
            log::error!("Audio::play_music: unknown sound handle");
            return;
        };
        let data = if looping {
            sound.0.loop_region(..)
        } else {
            sound.0.clone()
        };
        match manager.lock().unwrap().play(data) {
            Ok(handle) => self.music = Some(handle),
            Err(error) => log::error!("failed to play music: {error}"),
        }
    }

    /// Stop the music slot (if playing).
    pub fn stop_music(&mut self) {
        if let Some(mut handle) = self.music.take() {
            handle.stop(Tween::default());
        }
    }

    /// Master volume for everything, linear `0.0..=1.0`.
    pub fn set_master_volume(&mut self, linear: f32) {
        if let Some(manager) = &self.manager {
            manager
                .lock()
                .unwrap()
                .main_track()
                .set_volume(to_decibels(linear), Tween::default());
        }
    }
}

/// Parse encoded audio bytes into a [`Sound`]. Pure CPU; works without an audio device.
pub fn decode_sound(path: &str, bytes: Vec<u8>) -> Result<Sound, AssetError> {
    StaticSoundData::from_cursor(Cursor::new(bytes))
        .map(Sound)
        .map_err(|error| AssetError::Decode {
            path: path.to_string(),
            message: error.to_string(),
        })
}

/// One-line sound loading for game systems: `sounds.load("boom.ogg")`.
#[derive(SystemParam)]
pub struct SoundLoader<'w> {
    server: Res<'w, AssetServer>,
    sounds: ResMut<'w, Assets<Sound>>,
}

impl SoundLoader<'_> {
    /// Load a sound by path (relative to the asset root), deduplicated by path. Failures log
    /// and return a handle that plays nothing.
    pub fn load(&mut self, path: &str) -> Handle<Sound> {
        if let Some(handle) = self.sounds.handle_for_path(path) {
            return handle;
        }
        match self
            .server
            .read_bytes(path)
            .and_then(|bytes| decode_sound(path, bytes))
        {
            Ok(sound) => self.sounds.insert_with_path(path, sound),
            Err(error) => {
                log::error!("{error}; sound will not play");
                Handle::INVALID
            }
        }
    }
}

/// Installs the [`Audio`] resource and [`Sound`] storage. Part of `DefaultPlugins`.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Assets::<Sound>::default());
        app.world_mut().insert_resource(Audio::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal valid 8-sample mono 16-bit 22.05 kHz WAV.
    fn tiny_wav() -> Vec<u8> {
        let samples: [i16; 8] = [0, 8000, 16000, 8000, 0, -8000, -16000, -8000];
        let data_len = (samples.len() * 2) as u32;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&1u16.to_le_bytes()); // mono
        wav.extend_from_slice(&22050u32.to_le_bytes()); // sample rate
        wav.extend_from_slice(&44100u32.to_le_bytes()); // byte rate
        wav.extend_from_slice(&2u16.to_le_bytes()); // block align
        wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    #[test]
    fn decodes_wav_bytes() {
        let sound = decode_sound("tiny.wav", tiny_wav()).unwrap();
        assert_eq!(sound.0.num_frames(), 8);
    }

    #[test]
    fn garbage_bytes_error_names_the_path() {
        let Err(error) = decode_sound("bad.ogg", b"not audio".to_vec()) else {
            panic!("garbage decoded as audio");
        };
        assert!(error.to_string().contains("bad.ogg"));
    }

    #[test]
    fn engine_survives_headless_audio_and_bad_handles() {
        // Audio::new must not panic with or without a device; playing garbage handles logs
        // instead of crashing.
        let mut audio = Audio::new();
        let sounds = Assets::<Sound>::default();
        audio.play(&sounds, Handle::INVALID);
        audio.set_master_volume(0.5);
        audio.stop_music();
    }

    #[test]
    fn decibel_conversion_is_sane() {
        assert_eq!(to_decibels(1.0).0, 0.0);
        assert!(to_decibels(0.5).0 < 0.0);
        assert_eq!(to_decibels(0.0), Decibels::SILENCE);
    }
}
