use std::sync::Arc;
use std::time::Duration;

use chassis::Gpu;
use grind::Jobs;

use crate::backend::{AudioBackend, PhysicsBackend};
use crate::time::Time;

/// Everything the engine owns.
///
/// Data only: a plugin owns its own state and gets `&mut Engine` per hook, so
/// `adam` depends on no plugin crate. The substitutable fields are trait
/// objects chosen by the binary; `gpu` is `None` headless, which is the whole
/// of "headless is the same binary".
///
/// Field order is drop order: the GPU drops last so a plugin's pipelines and
/// buffers outlive the device that made them.
pub struct Engine {
    pub jobs: Arc<Jobs>,
    pub time: Time,
    pub events: pings::EventBus,
    pub input: eyes::Input,
    pub scene: meats::Scene,
    pub assets: vault::Assets,
    pub physics: Option<Box<dyn PhysicsBackend>>,
    pub audio: Option<Box<dyn AudioBackend>>,
    pub gpu: Option<Box<dyn Gpu>>,
}

impl Engine {
    /// Headless engine: no GPU, no swapchain, deterministic fixed steps.
    pub fn headless() -> Self {
        Self::new()
    }

    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Jobs::new()),
            time: Time::new(),
            events: pings::EventBus::new(),
            input: eyes::Input::new(),
            scene: meats::Scene::new(),
            assets: vault::Assets::new("."),
            physics: None,
            audio: None,
            gpu: None,
        }
    }

    pub fn with_gpu(gpu: Box<dyn Gpu>) -> Self {
        Self {
            gpu: Some(gpu),
            ..Self::new()
        }
    }

    /// Assets root, before any plugin is built.
    pub fn with_asset_root(root: impl Into<std::path::PathBuf>) -> Self {
        Self {
            assets: vault::Assets::new(root),
            ..Self::new()
        }
    }

    pub fn is_headless(&self) -> bool {
        self.gpu.is_none()
    }

    pub fn is_dirty(&self) -> bool {
        self.time.pending_steps() > 0
    }

    /// The data side of one fixed step: transforms, physics, asset polling.
    ///
    /// Runs *before* the plugins' `on_update` hooks, so a hook that reads a
    /// world transform reads the one it will be drawn with.
    pub fn simulate_fixed_step(&mut self, step: Duration) {
        self.scene.update_transforms();

        if let Some(physics) = self.physics.as_mut() {
            physics.step(&mut self.scene, step.as_secs_f32());
        }

        for (id, result) in self.assets.poll() {
            if let Err(error) = result {
                log::warn!("asset {id:?} failed to load: {error}");
            }
        }
    }

    pub fn frame_delta(&self) -> Duration {
        self.time.delta()
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
