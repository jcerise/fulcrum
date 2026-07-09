//! Headless tests for the app builder: schedules, plugins, resources, and events — using only
//! `fulcrum_core` paths, never `bevy_ecs` directly.

use fulcrum_core::{
    Commands, Component, Event, EventReader, EventWriter, Fulcrum, IntoScheduleConfigs, Plugin,
    Query, ResMut, Resource,
};

#[derive(Component)]
struct Counter(u32);

#[derive(Resource, Default)]
struct Total(u32);

fn spawn_counters(mut commands: Commands) {
    commands.spawn(Counter(0));
    commands.spawn(Counter(100));
}

fn increment(mut counters: Query<&mut Counter>, mut total: ResMut<Total>) {
    for mut counter in &mut counters {
        counter.0 += 1;
        total.0 += 1;
    }
}

#[test]
fn startup_and_fixed_systems_run_headless() {
    let mut app = Fulcrum::new("test")
        .insert_resource(Total::default())
        .add_startup(spawn_counters)
        .add_system(increment);

    app.run_startup();
    for _ in 0..5 {
        app.tick();
    }

    let world = app.world_mut();
    let mut values: Vec<u32> = world.query::<&Counter>().iter(world).map(|c| c.0).collect();
    values.sort_unstable();
    assert_eq!(
        values,
        vec![5, 105],
        "each counter incremented once per tick"
    );
    assert_eq!(world.resource::<Total>().0, 10, "2 counters x 5 ticks");
}

struct CounterPlugin;

impl Plugin for CounterPlugin {
    fn build(&self, app: &mut Fulcrum) {
        app.world_mut().insert_resource(Total::default());
        app.add_systems(fulcrum_core::Startup, spawn_counters);
        app.add_systems(fulcrum_core::FixedUpdate, increment);
    }
}

#[test]
fn plugins_add_systems_and_resources() {
    let mut app = Fulcrum::new("test").with_plugin(CounterPlugin);

    app.run_startup();
    app.tick();
    app.tick();

    assert_eq!(app.world().resource::<Total>().0, 4, "2 counters x 2 ticks");
}

#[derive(Event)]
struct Ping;

#[derive(Resource, Default)]
struct PingsSeen(u32);

fn send_ping(mut pings: EventWriter<Ping>) {
    pings.write(Ping);
}

fn count_pings(mut pings: EventReader<Ping>, mut seen: ResMut<PingsSeen>) {
    seen.0 += pings.read().count() as u32;
}

#[test]
fn events_flow_between_systems() {
    let mut app = Fulcrum::new("test")
        .insert_resource(PingsSeen::default())
        .add_event::<Ping>()
        .add_system((send_ping, count_pings).chain());

    app.run_startup();
    for _ in 0..5 {
        app.tick();
    }

    assert_eq!(app.world().resource::<PingsSeen>().0, 5);
}

#[test]
fn config_defaults_and_resource_access() {
    let app = Fulcrum::new("my title");
    assert_eq!(app.config().title, "my title");
    assert_eq!(app.config().tick_rate, 60);
    assert_eq!(app.config().seed, fulcrum_core::DEFAULT_SEED);
}
