//! The render graph facade: declaration, versioning, compilation.
//!
//! Backend-neutral: the compiler emits [`ImageBarrier`] rather than a Vulkan
//! barrier, so a backend lowers it and the whole compiler is testable with no
//! device, no driver and no Vulkan loader.
//!
//! The rule the facade exists to enforce:
//!
//! > A pass may only declare read access to a resource at a version it did not
//! > itself write in that same frame.
//!
//! That is not an optimisation. A resource written by pass 3 cannot be read by
//! pass 1, so no two passes touch the same resource version without the graph
//! knowing the order between them.

use std::sync::Arc;
use std::sync::Mutex;

pub mod builder;
pub mod compiler;
pub mod pass;
pub mod resources;
pub mod types;

use builder::RenderGraph as Builder;
use compiler::ResourceLifetime;
use resources::Resources;
use types::ValidationError;

pub use builder::{PassBuilder, RenderGraph};
pub use compiler::{compile, validate, CompiledGraph, CompiledPass};
pub use pass::{NoRecording, PassRecorder, PassScope, SlotTable};
pub use resources::{BufferSlot, TextureSlot, VersionEntry};
pub use types::{
    access_for_state, stage_for_state, transition, Access, BufferBarrier, BufferId,
    ColorAttachment, DepthAttachment, ErrorKind, ImageBarrier, ImageState, LoadOp, PassDesc,
    PassType, Queue, ResourceAccess, Stage, TextureId, NO_RESOURCE, WHOLE_SIZE,
};

/// A compiled frame: an immutable snapshot the executor can work from while the
/// frontend goes on editing the graph.
///
/// This is what keeps "declare" and "execute" from being the same lock. The
/// snapshot cannot change underneath the executor, so passes can be recorded in
/// parallel without holding the graph.
#[derive(Clone, Debug, Default)]
pub struct CompiledFrame {
    pub graph: CompiledGraph,
    pub resources: Resources,
    pub passes: Vec<PassDesc>,
}

/// The render graph.
///
/// `Clone` shares one graph behind a mutex: the frontend and backend both need
/// the graph in the same frame, and a deep-copying `Clone` would make the
/// unsynchronised case look safe.
#[derive(Clone)]
pub struct VulkanRenderGraph {
    inner: Arc<Mutex<Builder>>,
}

impl Default for VulkanRenderGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl VulkanRenderGraph {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Builder::new())),
        }
    }

    pub fn resources(&self) -> Resources {
        self.inner
            .lock()
            .expect("render graph mutex poisoned")
            .resources()
            .clone()
    }

    pub fn passes(&self) -> Vec<PassDesc> {
        self.inner
            .lock()
            .expect("render graph mutex poisoned")
            .passes()
            .to_vec()
    }

    /// Declare a pass and hand back a builder for it.
    ///
    /// The builder borrows the graph, so a pass cannot outlive the lock it was
    /// declared under. That is what stops a `&mut RenderGraph` escaping into
    /// something the compiler then reads while it is being changed.
    pub fn declare_pass<R>(&self, f: impl FnOnce(&mut PassBuilder<'_>) -> R) -> R {
        let mut guard = self.inner.lock().expect("render graph mutex poisoned");
        f(&mut guard.pass("pass"))
    }

    /// Compile the frame. The returned snapshot is independent of the graph, so
    /// the executor can record from it while the frontend builds the next frame.
    pub fn compile(&self) -> Result<CompiledFrame, Vec<ValidationError>> {
        let guard = self.inner.lock().expect("render graph mutex poisoned");
        let graph = guard.compile()?;
        Ok(CompiledFrame {
            graph,
            resources: guard.resources().clone(),
            passes: guard.passes().to_vec(),
        })
    }

    /// Validate without compiling. Cheaper, and what a debug layer calls every
    /// frame so the error names the pass rather than surfacing as a barrier bug.
    pub fn validate(&self) -> Vec<ValidationError> {
        self.inner
            .lock()
            .expect("render graph mutex poisoned")
            .validate()
    }

    /// Start a new frame: clear passes and the version log, keep nothing.
    ///
    /// Resources are re-declared each frame rather than cached. At graph scale
    /// that is free, and it means a resource removed from the frontend cannot
    /// linger in a pool waiting for an owner that no longer exists.
    pub fn begin_frame(&self) {
        self.inner
            .lock()
            .expect("render graph mutex poisoned")
            .reset();
    }

    /// The textures an executor must make resident, deduplicated across every
    /// pass. Two passes touching one texture is one allocation, not two.
    ///
    /// Read from the compiled barriers rather than the declared accesses: by the
    /// time a frame is compiled, an access that needed no transition has become
    /// an empty barrier. Collect them here and the executor allocates for a
    /// read-only resource with no transition that no pass actually touches.
    pub fn touched_textures(frame: &CompiledFrame) -> Vec<TextureId> {
        let mut seen: Vec<TextureId> = Vec::new();
        for pass in &frame.graph.passes {
            for color in &pass.colors {
                push_texture(&mut seen, color.texture);
                push_texture(&mut seen, color.resolve);
            }
            if let Some(depth) = &pass.depth {
                push_texture(&mut seen, depth.texture);
            }
            for barrier in pass.pre_barriers.iter().chain(pass.post_barriers.iter()) {
                push_texture(&mut seen, barrier.texture);
            }
        }
        seen
    }

    pub fn touched_buffers(frame: &CompiledFrame) -> Vec<BufferId> {
        let mut seen: Vec<BufferId> = Vec::new();
        for pass in &frame.graph.passes {
            for barrier in &pass.pre_buffer_barriers {
                if !seen.contains(&barrier.buffer) {
                    seen.push(barrier.buffer);
                }
            }
        }
        seen
    }

    /// The state each texture ends the frame in.
    ///
    /// One place to hand back to `present-src` or the executor's epilogue. The
    /// alternative is every consumer guessing, and a guess here is a layout that
    /// disagrees with the next frame's first barrier, which validation reports
    /// as a sync hazard far from the mistake.
    pub fn final_states(frame: &CompiledFrame) -> Vec<(TextureId, ImageState)> {
        let mut states: Vec<(TextureId, ImageState)> = Vec::new();
        for (index, slot) in frame.resources.textures().iter().enumerate() {
            let Some(last) = slot.version_log.last() else {
                continue;
            };
            if last.producer == u32::MAX {
                continue;
            }
            let id = index as TextureId;
            if !states.iter().any(|(existing, _)| *existing == id) {
                states.push((id, last.state));
            }
        }
        states
    }

    /// Transient aliasing candidates: textures whose lifetimes do not overlap,
    /// so they could share memory.
    ///
    /// Not yet acted on: aliasing needs barrier bookkeeping during recording, a
    /// separate piece of work. Reported so the frame dump can show what it buys.
    pub fn aliasing_candidates(frame: &CompiledFrame) -> Vec<(ResourceLifetime, ResourceLifetime)> {
        let lifetimes = &frame.graph.lifetimes;
        let mut out = Vec::new();
        for (i, a) in lifetimes.iter().enumerate() {
            for b in lifetimes.iter().skip(i + 1) {
                if a.version == 0 || b.version == 0 {
                    continue;
                }
                if a.first_use > b.last_use || b.first_use > a.last_use {
                    out.push((*a, *b));
                }
            }
        }
        out
    }

    /// The human-readable dump, for a debug layer or a test failure message.
    pub fn dump(&self) -> String {
        self.inner
            .lock()
            .expect("render graph mutex poisoned")
            .dump()
    }
}

fn push_texture(seen: &mut Vec<TextureId>, texture: TextureId) {
    if texture != NO_RESOURCE && !seen.contains(&texture) {
        seen.push(texture);
    }
}
