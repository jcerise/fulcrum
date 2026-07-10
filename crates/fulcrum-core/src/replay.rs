//! Command-level replay: recording, the `.freplay` file format, and playback with embedded
//! state-hash divergence checks.
//!
//! # The model
//!
//! Player intent enters the simulation through exactly two channels, both captured per tick:
//! the tick-sampled [`Input`](crate::Input) delta, and [`CommandEvent`]s. Commands are the
//! lockstep-shaped channel: anything a game (or the UI layer, or a Lua mod) wants replayed as
//! an *order* rather than raw input goes through [`CommandOutbox::send`]. Each tick the runner
//! drains the outbox into `Messages<CommandEvent>` for simulation systems to read — and, when
//! recording, into the replay. During playback locally-generated outbox commands are discarded
//! and the recorded stream is injected instead, so systems that deterministically re-derive
//! commands from replayed input never double them.
//!
//! Divergence detection: while recording, a [`StateHasher`] fingerprint of registered
//! simulation state is embedded every [`HASH_EVERY`] ticks; playback recomputes and compares
//! each one, and the first mismatch reports the tick number — the debugging entry point for
//! determinism bugs.

use std::path::Path;
use std::sync::Arc;

use bevy_ecs::message::Message;
use bevy_ecs::prelude::Resource;
use bevy_ecs::world::World;
use serde::{Deserialize, Serialize};

use crate::input::InputDelta;

/// Leading bytes of every `.freplay` file: "FREPLAY" + format byte.
pub const REPLAY_MAGIC: &[u8; 8] = b"FREPLAY\x01";

/// Bumped whenever the postcard-encoded layout changes.
pub const FORMAT_VERSION: u16 = 1;

/// A state hash is embedded (and checked) every this-many ticks.
pub const HASH_EVERY: u64 = 60;

/// A player command: an *order* to the simulation ("move these units here", "ui:click"). Read
/// them with `EventReader<CommandEvent>` in `FixedUpdate`; send them with
/// [`CommandOutbox::send`]. The payload is a RON string by convention (or any plain string).
#[derive(Message, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandEvent {
    /// Command name, e.g. `"move"` or `"ui:click"` (the `ui:` prefix is reserved).
    pub name: String,
    /// Command data as text (RON for structured payloads).
    pub payload: String,
}

/// Where commands are *sent*. Drained once per tick into `Messages<CommandEvent>` — and into
/// the replay when recording, or discarded in favor of the recorded stream during playback.
#[derive(Resource, Default)]
pub struct CommandOutbox(pub(crate) Vec<CommandEvent>);

impl CommandOutbox {
    /// Queue a command for the next simulation tick.
    pub fn send(&mut self, name: impl Into<String>, payload: impl Into<String>) {
        self.0.push(CommandEvent {
            name: name.into(),
            payload: payload.into(),
        });
    }
}

/// Everything that entered the simulation on one tick.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TickRecord {
    /// The tick's sampled input delta.
    pub input: InputDelta,
    /// Commands drained from the [`CommandOutbox`] this tick.
    pub commands: Vec<CommandEvent>,
}

/// Identifies what a replay was recorded against. Mismatches warn (or error) before playback.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplayHeader {
    /// [`FORMAT_VERSION`] at record time.
    pub format_version: u16,
    /// Engine crate version at record time.
    pub engine_version: String,
    /// The recording game's `FulcrumConfig::title`.
    pub game_id: String,
    /// Seed the recording run used; playback reseeds `SimRng` from it.
    pub seed: u64,
    /// Tick rate of the recording run. A mismatch makes playback meaningless.
    pub tick_rate: u32,
    /// Loaded mods `(id, version)` in load order (empty when not using mods).
    pub mods: Vec<(String, String)>,
}

/// A recorded run: header + per-tick records + periodic state hashes. Same-binary determinism
/// (see `docs/determinism.md`) means feeding the records back reproduces the run exactly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Replay {
    /// Record-time identity and settings.
    pub header: ReplayHeader,
    /// One record per tick, starting at tick 0.
    pub ticks: Vec<TickRecord>,
    /// `(tick, hash)` fingerprints taken *before* that tick's simulation ran.
    pub state_hashes: Vec<(u64, u64)>,
}

impl Replay {
    /// Write the replay as a `.freplay` file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ReplayError> {
        let mut bytes = REPLAY_MAGIC.to_vec();
        bytes.extend(postcard::to_stdvec(self)?);
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Read a `.freplay` file.
    pub fn load(path: impl AsRef<Path>) -> Result<Replay, ReplayError> {
        let bytes = std::fs::read(path)?;
        let body = bytes
            .strip_prefix(REPLAY_MAGIC.as_slice())
            .ok_or(ReplayError::BadMagic)?;
        Ok(postcard::from_bytes(body)?)
    }
}

/// Why recording, saving, loading, or playback failed.
#[derive(thiserror::Error, Debug)]
pub enum ReplayError {
    /// Reading or writing the file failed.
    #[error("replay io error: {0}")]
    Io(#[from] std::io::Error),
    /// The file doesn't start with [`REPLAY_MAGIC`].
    #[error("not a .freplay file (bad magic bytes)")]
    BadMagic,
    /// Encoding or decoding the postcard body failed (usually a format-version mismatch).
    #[error("replay encode/decode error: {0}")]
    Codec(#[from] postcard::Error),
    /// Playback state stopped matching the recording. The tick is where simulation state was
    /// first *observed* wrong; the actual divergence happened in the [`HASH_EVERY`] ticks
    /// before it.
    #[error("replay diverged at tick {tick}: recorded hash {expected:#018x}, got {actual:#018x}")]
    Divergence {
        /// Tick whose pre-simulation state hash mismatched.
        tick: u64,
        /// Hash embedded in the replay.
        expected: u64,
        /// Hash computed during playback.
        actual: u64,
    },
}

/// Captures tick records while active. Inserted automatically;
/// [`FulcrumConfig::record_replays`](crate::FulcrumConfig::record_replays) starts it from tick
/// 0, or call [`start_recording`](Self::start_recording) (before the first tick — a replay
/// must start at tick 0 to be reproducible from startup).
#[derive(Resource, Default)]
pub struct ReplayRecorder {
    active: bool,
    pub(crate) ticks: Vec<TickRecord>,
    pub(crate) state_hashes: Vec<(u64, u64)>,
    pub(crate) started_at_tick: u64,
}

impl ReplayRecorder {
    pub(crate) fn new(active: bool) -> Self {
        Self {
            active,
            ..Self::default()
        }
    }

    /// Begin capturing (clears any previous recording).
    pub fn start_recording(&mut self) {
        self.active = true;
        self.ticks.clear();
        self.state_hashes.clear();
        self.started_at_tick = 0;
    }

    /// Stop capturing. The buffered recording remains available to [`save_replay`].
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Is a recording in progress?
    pub fn is_recording(&self) -> bool {
        self.active
    }

    /// Number of ticks captured so far.
    pub fn len(&self) -> usize {
        self.ticks.len()
    }

    /// True when nothing has been captured.
    pub fn is_empty(&self) -> bool {
        self.ticks.is_empty()
    }
}

/// Write the recording captured so far to `path` (the recorder keeps running — call
/// [`ReplayRecorder::stop`] to end it). Callable from an exclusive system (`&mut World`) or via
/// [`Fulcrum::save_replay`](crate::Fulcrum::save_replay). A final state hash of the current
/// world is appended so playback verifies the end state too.
pub fn save_replay(world: &mut World, path: impl AsRef<Path>) -> Result<(), ReplayError> {
    let final_hash = world
        .get_resource::<StateHasher>()
        .cloned()
        .map(|hasher| (hasher.0)(world));
    let config = world.resource::<crate::FulcrumConfig>();
    let header = ReplayHeader {
        format_version: FORMAT_VERSION,
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        game_id: config.title.clone(),
        seed: config.seed,
        tick_rate: config.tick_rate,
        mods: world
            .get_resource::<ReplayModSet>()
            .map(|m| m.0.clone())
            .unwrap_or_default(),
    };
    let tick = world.resource::<crate::Time>().tick;
    let recorder = world.resource::<ReplayRecorder>();
    if recorder.started_at_tick != 0 {
        log::warn!(
            "replay recording started at tick {} — playback reproduces runs only from tick 0",
            recorder.started_at_tick
        );
    }
    let mut state_hashes = recorder.state_hashes.clone();
    if let Some(hash) = final_hash {
        state_hashes.push((tick, hash));
    }
    let replay = Replay {
        header,
        ticks: recorder.ticks.clone(),
        state_hashes,
    };
    log::info!(
        "saving replay: {} ticks, {} state hashes",
        replay.ticks.len(),
        replay.state_hashes.len()
    );
    replay.save(path)
}

/// The loaded mod set `(id, version)` in load order, inserted by `ModPlugin` (or by hand) so
/// replays can record — and playback can verify — what was loaded.
#[derive(Resource, Default, Clone, Debug, PartialEq)]
pub struct ReplayModSet(pub Vec<(String, String)>);

/// Playback state. Inserted (idle) in every app — resources are entities in the ECS, so
/// inserting it only for playback would shift every subsequently-spawned entity id relative to
/// the recording run and diverge instantly. While active, each tick consumes the next
/// [`TickRecord`] instead of live input, and embedded state hashes are checked on the fly.
#[derive(Resource, Default)]
pub struct ReplayPlayback {
    pub(crate) replay: Option<Replay>,
    pub(crate) cursor: usize,
    pub(crate) error: Option<ReplayError>,
}

impl ReplayPlayback {
    /// Begin playing `replay` from its first tick.
    pub(crate) fn begin(&mut self, replay: Replay) {
        self.replay = Some(replay);
        self.cursor = 0;
        self.error = None;
    }

    /// Is a playback consuming ticks right now?
    pub fn active(&self) -> bool {
        self.replay.is_some() && !self.finished()
    }

    /// All records consumed, or playback stopped on a divergence. False when idle.
    pub fn finished(&self) -> bool {
        match &self.replay {
            Some(replay) => self.cursor >= replay.ticks.len() || self.error.is_some(),
            None => false,
        }
    }

    /// The divergence (or other error) that stopped playback, if any.
    pub fn error(&self) -> Option<&ReplayError> {
        self.error.as_ref()
    }

    /// `(ticks_played, ticks_total)`. `(0, 0)` when idle.
    pub fn progress(&self) -> (usize, usize) {
        (
            self.cursor,
            self.replay.as_ref().map(|r| r.ticks.len()).unwrap_or(0),
        )
    }
}

/// Fingerprints registered simulation state as a `u64`. Installed by `ScenePlugin` (which owns
/// the component registry the hash walks); without it, replays record and play back but carry
/// no divergence checks. Covers *registered* components plus `Time::tick` and the `SimRng`
/// stream position — unregistered and cosmetic state is invisible to it.
#[derive(Resource, Clone)]
pub struct StateHasher(pub Arc<dyn Fn(&mut World) -> u64 + Send + Sync>);
