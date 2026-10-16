use std::time::Duration;

use crate::engine::Engine;
use crate::error::Result;

/// Per-frame plumbing handed to every hook.
pub struct FrameCtx {
    /// Index of the frame being executed, starting at 1.
    pub frame: u64,
    /// Real time since the previous frame, unscaled.
    pub delta: Duration,
    /// The fixed step this hook runs under: `delta` on render frames,
    /// `fixed_dt` on each simulation substep.
    pub step: Duration,
    /// Simulation speed multiplier in effect.
    pub scale: f32,
    /// Set by any hook to end the loop after this frame.
    pub quit: bool,
}

impl FrameCtx {
    pub fn step_seconds(&self) -> f32 {
        self.step.as_secs_f32()
    }
}

/// A unit of engine behavior.
///
/// Plugins are compiled in, not discovered at runtime; the swappable pieces (a
/// model loader, a physics backend, a UI toolkit) are sibling crates picked by
/// a Cargo feature.
///
/// The registry lives on `App`, beside `Engine`, never inside it: iterating
/// plugins and borrowing the engine must stay disjoint.
pub trait Plugin: Send + 'static {
    /// Plugin identifier, for `--list-plugins` and the boot log.
    ///
    /// A method rather than `const NAME`: an associated const would make the
    /// trait not dyn-compatible.
    fn name(&self) -> &'static str;

    /// Populate the engine's data from this plugin. Called once, in
    /// registration order.
    fn build(&mut self, eng: &mut Engine) -> Result<()>;

    fn on_start(&mut self, _eng: &mut Engine) -> Result<()> {
        Ok(())
    }

    /// Simulation substep. Called once per fixed step, not once per frame.
    fn on_update(&mut self, _eng: &mut Engine, _ctx: &mut FrameCtx) {}

    /// Once per rendered frame, after the substeps and before `on_render`.
    /// Frame-rate logic belongs here: `on_update` cannot serve it, because a
    /// frame that reaches no fixed step runs no substeps at all.
    fn on_frame(&mut self, _eng: &mut Engine, _ctx: &mut FrameCtx) {}

    /// Record this frame's GPU work. Only called when `Engine::gpu` is `Some`.
    fn on_render(&mut self, _eng: &mut Engine, _ctx: &mut FrameCtx) {}

    fn on_stop(&mut self, _eng: &mut Engine) {}
}
