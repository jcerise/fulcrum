//! Animation state machines: data-driven `idle -> run when moving` logic, so every game stops
//! hand-rolling the same match statement.
//!
//! ```ron
//! StateMachine(
//!     initial: "idle",
//!     params: { "speed": Float(0.0), "attack": Trigger },
//!     states: {
//!         "idle":   (clip: "player.json#idle"),
//!         "run":    (clip: "player.json#run"),
//!         "attack": (clip: "player.json#attack", on_finish: "idle"),
//!     },
//!     transitions: [
//!         (from: State("idle"), to: "run",    when: [Gt("speed", 0.1)]),
//!         (from: State("run"),  to: "idle",   when: [Lt("speed", 0.1)]),
//!         (from: Any,           to: "attack", when: [Triggered("attack")]),
//!     ],
//! )
//! ```
//!
//! Runs in `FixedUpdate` (deterministic): game systems set params, the machine evaluates
//! transitions — `Any` first, then the current state's, declaration order, first match wins —
//! and switches the entity's [`AnimationPlayer`]. Triggers last exactly one tick.

use std::collections::BTreeMap;

use bevy_ecs::prelude::{Component, Query, Res};
use bevy_ecs::system::SystemParam;
use fulcrum_asset::{AssetError, AssetServer, Assets, Handle};
use fulcrum_core::{FxHashMap, FxHashSet};
use serde::Deserialize;

use crate::aseprite::AsepriteLoader;
use crate::clip::AnimationClip;
use crate::player::AnimationPlayer;

#[derive(Deserialize)]
#[serde(rename = "StateMachine")]
struct MachineDef {
    initial: String,
    #[serde(default)]
    params: BTreeMap<String, ParamDef>,
    states: BTreeMap<String, StateDef>,
    #[serde(default)]
    transitions: Vec<TransitionDef>,
}

#[derive(Deserialize)]
enum ParamDef {
    /// A float parameter with its default value.
    Float(f32),
    /// A bool parameter with its default value.
    Bool(bool),
    /// A one-tick trigger.
    Trigger,
}

#[derive(Deserialize)]
struct StateDef {
    clip: String,
    #[serde(default)]
    on_finish: Option<String>,
}

#[derive(Deserialize)]
struct TransitionDef {
    from: FromDef,
    to: String,
    #[serde(default)]
    when: Vec<Condition>,
}

#[derive(Deserialize)]
enum FromDef {
    /// Checked from every state (before the state's own transitions).
    Any,
    /// Checked only from the named state.
    State(String),
}

/// A transition condition; a transition's `when` list is AND-ed.
#[derive(Deserialize, Clone, Debug)]
pub enum Condition {
    /// Float param strictly greater than the value.
    Gt(String, f32),
    /// Float param strictly less than the value.
    Lt(String, f32),
    /// Bool param equals the value.
    Is(String, bool),
    /// Trigger param fired this tick.
    Triggered(String),
}

struct Transition {
    to: usize,
    when: Vec<Condition>,
}

/// A validated, clip-resolved state machine.
pub struct StateMachineAsset {
    initial: usize,
    state_names: Vec<String>,
    clips: Vec<Handle<AnimationClip>>,
    on_finish: Vec<Option<usize>>,
    any_transitions: Vec<Transition>,
    per_state: Vec<Vec<Transition>>,
    float_defaults: FxHashMap<String, f32>,
    bool_defaults: FxHashMap<String, bool>,
}

/// Plays a [`StateMachineAsset`] on the entity's [`AnimationPlayer`]. Game systems talk to it
/// exclusively through params: [`set_float`](Self::set_float), [`set_bool`](Self::set_bool),
/// [`trigger`](Self::trigger).
#[derive(Component)]
pub struct Animator {
    /// The machine to run.
    pub machine: Handle<StateMachineAsset>,
    state: Option<usize>,
    state_name: String,
    floats: FxHashMap<String, f32>,
    bools: FxHashMap<String, bool>,
    triggers: FxHashSet<String>,
}

impl Animator {
    /// Start (or restart) at the machine's initial state on the next tick.
    pub fn new(machine: Handle<StateMachineAsset>) -> Self {
        Self {
            machine,
            state: None,
            state_name: String::new(),
            floats: FxHashMap::default(),
            bools: FxHashMap::default(),
            triggers: FxHashSet::default(),
        }
    }

    /// Set a float parameter.
    pub fn set_float(&mut self, name: &str, value: f32) {
        self.floats.insert(name.to_string(), value);
    }

    /// Set a bool parameter.
    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.bools.insert(name.to_string(), value);
    }

    /// Fire a trigger (consumed at the end of the current tick's evaluation).
    pub fn trigger(&mut self, name: &str) {
        self.triggers.insert(name.to_string());
    }

    /// The current state's name (empty until the first tick).
    pub fn state(&self) -> &str {
        &self.state_name
    }

    fn condition_met(&self, condition: &Condition) -> bool {
        match condition {
            Condition::Gt(name, value) => self.floats.get(name).is_some_and(|v| v > value),
            Condition::Lt(name, value) => self.floats.get(name).is_some_and(|v| v < value),
            Condition::Is(name, value) => self.bools.get(name).is_some_and(|v| v == value),
            Condition::Triggered(name) => self.triggers.contains(name),
        }
    }
}

/// Parse + validate machine RON, resolving clip references through `resolve_clip`. Collects
/// every problem, not just the first.
pub(crate) fn build_machine(
    path: &str,
    source: &str,
    mut resolve_clip: impl FnMut(&str) -> Result<Handle<AnimationClip>, String>,
) -> Result<StateMachineAsset, AssetError> {
    let def: MachineDef = ron::Options::default()
        .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
        .from_str(source)
        .map_err(|error| AssetError::Decode {
            path: path.to_string(),
            message: error.to_string(),
        })?;

    let mut errors: Vec<String> = Vec::new();
    let state_names: Vec<String> = def.states.keys().cloned().collect();
    let index_of = |name: &str| state_names.iter().position(|n| n == name);

    let initial = index_of(&def.initial).unwrap_or_else(|| {
        errors.push(format!("initial state `{}` does not exist", def.initial));
        0
    });

    let mut float_defaults = FxHashMap::default();
    let mut bool_defaults = FxHashMap::default();
    let mut trigger_names = FxHashSet::default();
    for (name, param) in &def.params {
        match param {
            ParamDef::Float(value) => {
                float_defaults.insert(name.clone(), *value);
            }
            ParamDef::Bool(value) => {
                bool_defaults.insert(name.clone(), *value);
            }
            ParamDef::Trigger => {
                trigger_names.insert(name.clone());
            }
        }
    }

    let mut clips = Vec::new();
    let mut on_finish = Vec::new();
    for (state, state_def) in &def.states {
        match resolve_clip(&state_def.clip) {
            Ok(clip) => clips.push(clip),
            Err(message) => {
                errors.push(format!("state `{state}`: {message}"));
                clips.push(Handle::INVALID);
            }
        }
        on_finish.push(match &state_def.on_finish {
            Some(target) => {
                let index = index_of(target);
                if index.is_none() {
                    errors.push(format!(
                        "state `{state}`: on_finish `{target}` does not exist"
                    ));
                }
                index
            }
            None => None,
        });
    }

    let check_condition = |condition: &Condition, context: &str| -> Option<String> {
        let (name, expected) = match condition {
            Condition::Gt(name, _) | Condition::Lt(name, _) => (name, "Float"),
            Condition::Is(name, _) => (name, "Bool"),
            Condition::Triggered(name) => (name, "Trigger"),
        };
        let declared = match expected {
            "Float" => float_defaults.contains_key(name),
            "Bool" => bool_defaults.contains_key(name),
            _ => trigger_names.contains(name),
        };
        (!declared).then(|| format!("{context}: param `{name}` is not declared as {expected}"))
    };

    let mut any_transitions = Vec::new();
    let mut per_state: Vec<Vec<Transition>> = state_names.iter().map(|_| Vec::new()).collect();
    for (i, transition) in def.transitions.iter().enumerate() {
        let context = format!("transition #{i} (to `{}`)", transition.to);
        let Some(to) = index_of(&transition.to) else {
            errors.push(format!("{context}: target state does not exist"));
            continue;
        };
        for condition in &transition.when {
            errors.extend(check_condition(condition, &context));
        }
        let built = Transition {
            to,
            when: transition.when.clone(),
        };
        match &transition.from {
            FromDef::Any => any_transitions.push(built),
            FromDef::State(from) => match index_of(from) {
                Some(from) => per_state[from].push(built),
                None => errors.push(format!("{context}: from state `{from}` does not exist")),
            },
        }
    }

    if !errors.is_empty() {
        return Err(AssetError::Decode {
            path: path.to_string(),
            message: errors.join("; "),
        });
    }
    Ok(StateMachineAsset {
        initial,
        state_names,
        clips,
        on_finish,
        any_transitions,
        per_state,
        float_defaults,
        bool_defaults,
    })
}

/// Test hook: build a machine from RON with a custom clip resolver.
pub fn test_build(
    source: &str,
    resolve_clip: impl FnMut(&str) -> Result<Handle<AnimationClip>, String>,
) -> Result<StateMachineAsset, AssetError> {
    build_machine("<test>", source, resolve_clip)
}

/// `FixedUpdate` system (before clip advance): evaluate every animator's transitions.
pub(crate) fn drive_animators(
    mut animators: Query<(&mut Animator, &mut AnimationPlayer)>,
    machines: Res<Assets<StateMachineAsset>>,
) {
    for (mut animator, mut player) in &mut animators {
        let Some(machine) = machines.get(animator.machine) else {
            continue;
        };
        // First run: apply param defaults (without clobbering already-set values) and enter the
        // initial state.
        if animator.state.is_none() {
            for (name, value) in &machine.float_defaults {
                animator.floats.entry(name.clone()).or_insert(*value);
            }
            for (name, value) in &machine.bool_defaults {
                animator.bools.entry(name.clone()).or_insert(*value);
            }
            enter(&mut animator, &mut player, machine, machine.initial);
        }
        let current = animator.state.unwrap_or(machine.initial);

        // Any-state transitions first (never re-entering the current state), then the current
        // state's own, declaration order, first match wins.
        let mut next = machine
            .any_transitions
            .iter()
            .filter(|t| t.to != current)
            .chain(machine.per_state[current].iter())
            .find(|t| t.when.iter().all(|c| animator.condition_met(c)))
            .map(|t| t.to);
        if next.is_none() && player.finished() {
            next = machine.on_finish[current];
        }
        if let Some(next) = next
            && next != current
        {
            enter(&mut animator, &mut player, machine, next);
        }
        // Triggers last one tick.
        animator.triggers.clear();
    }
}

fn enter(
    animator: &mut Animator,
    player: &mut AnimationPlayer,
    machine: &StateMachineAsset,
    state: usize,
) {
    animator.state = Some(state);
    animator.state_name = machine.state_names[state].clone();
    *player = AnimationPlayer::play(machine.clips[state]);
}

/// One-line machine loading: `let machine = animators.load("player.animsm.ron")?;`
#[derive(SystemParam)]
pub struct AnimatorLoader<'w> {
    server: Res<'w, AssetServer>,
    machines: bevy_ecs::prelude::ResMut<'w, Assets<StateMachineAsset>>,
    aseprite: AsepriteLoader<'w>,
}

impl AnimatorLoader<'_> {
    /// Load and validate a `.animsm.ron` file; clip references (`file.json#tag`) load their
    /// Aseprite files on demand.
    pub fn load(&mut self, path: &str) -> Result<Handle<StateMachineAsset>, AssetError> {
        if let Some(handle) = self.machines.handle_for_path(path) {
            return Ok(handle);
        }
        let bytes = self.server.read_bytes(path)?;
        let source = String::from_utf8_lossy(&bytes).into_owned();
        let aseprite = &mut self.aseprite;
        let machine = build_machine(path, &source, |clip_ref| {
            let (file, tag) = clip_ref
                .split_once('#')
                .ok_or_else(|| format!("clip `{clip_ref}` is not `file.json#tag`"))?;
            let import = aseprite.load(file).map_err(|e| e.to_string())?;
            import
                .clips
                .get(tag)
                .copied()
                .ok_or_else(|| format!("`{file}` has no tag `{tag}`"))
        })?;
        Ok(self.machines.insert_with_path(path, machine))
    }
}
