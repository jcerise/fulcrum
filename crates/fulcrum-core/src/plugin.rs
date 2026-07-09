//! The [`Plugin`] trait: Fulcrum's unit of engine and game composition.

use crate::app::Fulcrum;

/// A group of systems, resources, and configuration installed into a [`Fulcrum`] app with
/// [`Fulcrum::with_plugin`].
///
/// Engine features (windowing, rendering, audio, ...) are plugins; games can define their own to
/// organize large projects.
///
/// ```
/// use fulcrum_core::{Fulcrum, Plugin};
///
/// struct ScorePlugin;
///
/// impl Plugin for ScorePlugin {
///     fn build(&self, app: &mut Fulcrum) {
///         // add systems and resources here
///         let _ = app;
///     }
/// }
///
/// let app = Fulcrum::new("demo").with_plugin(ScorePlugin);
/// ```
pub trait Plugin {
    /// Configure the app: add systems, insert resources, register events, or install a runner.
    fn build(&self, app: &mut Fulcrum);
}
