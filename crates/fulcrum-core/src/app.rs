//! The [`Fulcrum`] app builder: the engine's front door.

use bevy_ecs::message::Message;
use bevy_ecs::prelude::{Messages, Resource, Schedule, Schedules};
use bevy_ecs::schedule::{IntoScheduleConfigs, ScheduleLabel};
use bevy_ecs::system::ScheduleSystem;
use bevy_ecs::world::World;

use crate::Color;
use crate::plugin::Plugin;
use crate::schedule::{FixedUpdate, PreRender, Startup, Update};
use crate::time::Time;

/// Default simulation RNG seed used when [`FulcrumConfig::seed`] is not overridden.
pub const DEFAULT_SEED: u64 = 0xF0C0_55ED;

/// Startup configuration for a [`Fulcrum`] app. Also inserted into the world as a resource, so
/// systems and plugins can read it.
#[derive(Resource, Clone, Debug)]
pub struct FulcrumConfig {
    /// Window title.
    pub title: String,
    /// Initial window size in physical pixels.
    pub window_size: (u32, u32),
    /// Simulation tick rate in Hz. The fixed timestep is `1.0 / tick_rate`.
    pub tick_rate: u32,
    /// Seed for the deterministic simulation RNG (`SimRng`).
    pub seed: u64,
    /// Color the window is cleared to each frame.
    pub clear_color: Color,
    /// Whether debug gizmos draw. Defaults to true in debug builds, false in release, so
    /// shipped games don't pay for stray debug overlays.
    pub gizmos_enabled: bool,
}

impl Default for FulcrumConfig {
    fn default() -> Self {
        Self {
            title: "Fulcrum".to_string(),
            window_size: (1280, 720),
            tick_rate: 60,
            seed: DEFAULT_SEED,
            clear_color: Color::BLACK,
            gizmos_enabled: cfg!(debug_assertions),
        }
    }
}

/// A runner takes ownership of the built app and drives it. `fulcrum-render`'s window plugin
/// installs the real winit runner; without one, [`Fulcrum::run`] falls back to a headless runner
/// that executes [`Startup`] and returns (tests drive ticks directly via [`Fulcrum::tick`]).
type Runner = Box<dyn FnOnce(Fulcrum)>;

/// The Fulcrum app: an ECS world plus schedules, built with a fluent API and started with
/// [`run`](Fulcrum::run).
///
/// ```
/// use fulcrum_core::{Commands, Fulcrum};
///
/// fn setup(mut _commands: Commands) { /* spawn things */ }
///
/// let mut app = Fulcrum::new("my game").add_startup(setup);
/// app.run_startup(); // headless; a windowed runner calls this for you inside `run()`
/// ```
pub struct Fulcrum {
    world: World,
    runner: Option<Runner>,
    /// Per-tick updaters for registered event types (double-buffered message queues).
    event_updaters: Vec<fn(&mut World)>,
}

impl Fulcrum {
    /// Create an app with the given window title and default configuration.
    pub fn new(title: impl Into<String>) -> Self {
        Self::with_config(FulcrumConfig {
            title: title.into(),
            ..FulcrumConfig::default()
        })
    }

    /// Create an app with explicit configuration.
    pub fn with_config(config: FulcrumConfig) -> Self {
        let mut world = World::new();
        let mut schedules = Schedules::default();
        schedules.insert(Schedule::new(Startup));
        schedules.insert(Schedule::new(FixedUpdate));
        schedules.insert(Schedule::new(Update));
        schedules.insert(Schedule::new(PreRender));
        world.insert_resource(schedules);
        world.insert_resource(Time::new(config.tick_rate));
        world.insert_resource(crate::input::Input::default());
        world.insert_resource(crate::rng::SimRng::seeded(config.seed));
        world.insert_resource(config);
        Self {
            world,
            runner: None,
            event_updaters: Vec::new(),
        }
    }

    /// Install a [`Plugin`].
    pub fn with_plugin(mut self, plugin: impl Plugin) -> Self {
        plugin.build(&mut self);
        self
    }

    /// Add systems to [`Startup`], run once before the first tick.
    pub fn add_startup<M>(mut self, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) -> Self {
        self.add_systems(Startup, systems);
        self
    }

    /// Add systems to [`FixedUpdate`] — the deterministic simulation tick. This is where game
    /// logic belongs.
    pub fn add_system<M>(mut self, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) -> Self {
        self.add_systems(FixedUpdate, systems);
        self
    }

    /// Add systems to [`Update`], run once per rendered frame. Cosmetic work only.
    pub fn add_frame_system<M>(
        mut self,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> Self {
        self.add_systems(Update, systems);
        self
    }

    /// Insert a resource into the world.
    pub fn insert_resource<R: Resource>(mut self, resource: R) -> Self {
        self.world.insert_resource(resource);
        self
    }

    /// Register an event type, enabling `EventWriter<E>` / `EventReader<E>` system parameters.
    ///
    /// Event queues are drained on a two-tick cycle, advanced at the start of every simulation
    /// tick: an event sent during one tick is readable for the remainder of that tick and the
    /// whole next one.
    pub fn add_event<E: Message>(mut self) -> Self {
        self.register_event::<E>();
        self
    }

    /// Non-consuming form of [`add_event`](Self::add_event), for use inside plugins.
    pub fn register_event<E: Message>(&mut self) {
        if self.world.get_resource::<Messages<E>>().is_none() {
            self.world.init_resource::<Messages<E>>();
            self.event_updaters
                .push(|world| world.resource_mut::<Messages<E>>().update());
        }
    }

    /// Add systems to an arbitrary schedule. Non-consuming, for use inside plugins; games
    /// normally use [`add_startup`](Self::add_startup) / [`add_system`](Self::add_system) /
    /// [`add_frame_system`](Self::add_frame_system).
    pub fn add_systems<M>(
        &mut self,
        label: impl ScheduleLabel,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.world
            .resource_mut::<Schedules>()
            .add_systems(label, systems);
        self
    }

    /// The app's configuration (also available to systems as `Res<FulcrumConfig>`).
    pub fn config(&self) -> &FulcrumConfig {
        self.world.resource::<FulcrumConfig>()
    }

    /// Direct world access, for plugins and tests.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Direct mutable world access, for plugins and tests.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Install the function that will drive the app when [`run`](Self::run) is called. Called by
    /// the window plugin; games don't use this directly.
    pub fn set_runner(&mut self, runner: impl FnOnce(Fulcrum) + 'static) {
        self.runner = Some(Box::new(runner));
    }

    /// Hand the app to its runner. With no runner installed (headless), executes [`Startup`] and
    /// returns.
    pub fn run(mut self) {
        match self.runner.take() {
            Some(runner) => runner(self),
            None => {
                log::info!(
                    "no runner installed (fulcrum-render's window plugin provides one); \
                     running Startup headless and exiting"
                );
                self.run_startup();
            }
        }
    }

    /// Run the [`Startup`] schedule once. Runners call this before the first tick; headless
    /// tests call it directly.
    pub fn run_startup(&mut self) {
        self.world.run_schedule(Startup);
    }

    /// Advance the simulation by exactly one fixed tick: snapshot transforms for render
    /// interpolation, update registered event queues, run [`FixedUpdate`], and increment
    /// [`Time::tick`]. This is the canonical tick used by runners and headless tests alike.
    pub fn tick(&mut self) {
        crate::transform::snapshot_previous_transforms(&mut self.world);
        for update in &self.event_updaters {
            update(&mut self.world);
        }
        self.world.run_schedule(FixedUpdate);
        self.world.resource_mut::<Time>().tick += 1;
    }
}
