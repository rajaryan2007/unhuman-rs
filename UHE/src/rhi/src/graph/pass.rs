//! What a backend hands a pass, and what a pass hands back.
//!
//! `PassScope`, `PassRecorder` and `SlotTable` are the seam between the renderer
//! and a backend. A renderer implements `PassRecorder`; a backend calls it once
//! per pass with a scope, so a wgpu executor and a Vulkan one are interchangeable
//! from the renderer's side.

use crate::{Buffer, Texture};

/// Graph slot index to the handle the RHI uses.
///
/// The graph names resources by slot (`TextureId` is a `u32`) and the RHI names
/// them by generational handle. This is the translation.
///
/// It has to live somewhere a backend and the renderer can both reach, and it is
/// per-frame state rather than per-resource: the graph is rebuilt and recompiled
/// every frame, so a slot's handle can change without the slot index moving.
#[derive(Clone, Debug, Default)]
pub struct SlotTable {
    textures: Vec<Texture>,
    buffers: Vec<Buffer>,
}

impl SlotTable {
    /// Set a slot's texture. Grows the table, filling the gap with null handles
    /// so a slot can be assigned out of declaration order.
    pub fn set_texture(&mut self, slot: u32, handle: Texture) {
        if self.textures.len() <= slot as usize {
            self.textures.resize(slot as usize + 1, Texture::null());
        }
        self.textures[slot as usize] = handle;
    }

    pub fn set_buffer(&mut self, slot: u32, handle: Buffer) {
        if self.buffers.len() <= slot as usize {
            self.buffers.resize(slot as usize + 1, Buffer::null());
        }
        self.buffers[slot as usize] = handle;
    }

    /// `None` for a slot nothing was ever assigned, which is a clean error
    /// rather than a null handle reaching the driver.
    pub fn texture(&self, slot: u32) -> Option<Texture> {
        self.textures
            .get(slot as usize)
            .copied()
            .filter(|h| !h.is_null())
    }

    pub fn buffer(&self, slot: u32) -> Option<Buffer> {
        self.buffers
            .get(slot as usize)
            .copied()
            .filter(|h| !h.is_null())
    }

    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }
}

/// What one pass is told before it records.
///
/// Deliberately narrow. A pass that could reach the whole graph could record work
/// that belongs to another pass, and then the graph's ordering guarantee would
/// cover the barriers but not the draws, which is the bug the graph exists to
/// prevent.
pub struct PassScope<'a> {
    /// The pass's position in the compiled order. Also its identity.
    pub index: u32,
    pub name: &'a str,
    pub extent: crate::Extent2d,
    pub slots: &'a SlotTable,
}

impl<'a> PassScope<'a> {
    /// The handle behind a graph slot, or `None` if nothing was assigned.
    ///
    /// `None` rather than a null handle: a null handle that reaches a backend is
    /// either an ignored binding or a driver error about handle `VK_NULL_HANDLE`,
    /// and neither says which pass failed to declare it.
    pub fn texture(&self, slot: u32) -> Option<Texture> {
        self.slots.texture(slot)
    }

    pub fn buffer(&self, slot: u32) -> Option<Buffer> {
        self.slots.buffer(slot)
    }
}

/// Records one pass into one command list.
///
/// `Send + Sync` because a backend may call it from worker threads, so a frontend
/// that cannot share its recording code across threads cannot use that path. That
/// is a real constraint, not something to paper over with a mutex.
pub trait PassRecorder: Send + Sync {
    fn record(
        &self,
        scope: &PassScope<'_>,
        list: &mut dyn crate::CommandList,
    ) -> Result<(), crate::Error>;
}

impl<F> PassRecorder for F
where
    F: Fn(&PassScope<'_>, &mut dyn crate::CommandList) -> Result<(), crate::Error> + Send + Sync,
{
    fn record(
        &self,
        scope: &PassScope<'_>,
        list: &mut dyn crate::CommandList,
    ) -> Result<(), crate::Error> {
        self(scope, list)
    }
}

/// Records nothing.
///
/// For a frame that only does transfers, and for tests that want the submit
/// machinery without a renderer. A `Gpu::submit` with no frontend attached still
/// has to work, and this is how.
pub struct NoRecording;

impl PassRecorder for NoRecording {
    fn record(
        &self,
        _scope: &PassScope<'_>,
        _list: &mut dyn crate::CommandList,
    ) -> Result<(), crate::Error> {
        Ok(())
    }
}
