//! Composition: the scene colour and the occlusion into the swapchain image.
//!
//! The AO texture cannot be applied in the pass that reads it, because the graph
//! refuses a pass that reads and writes one resource (`compiler.rs`,
//! `ErrorKind::PassStateConflict`); the 2D and text slice draw into the scene
//! colour, so both are resolved here. One full-screen triangle, no vertex buffer.

use chassis::{BindGroup, CommandList, PassScope, Pipeline};

/// Bind group number within the composite shader.
pub const COMPOSITE_SET: u32 = 0;

pub struct CompositePass {
    pipeline: Pipeline,
    group: BindGroup,
}

impl CompositePass {
    pub fn new(pipeline: Pipeline, group: BindGroup) -> Self {
        Self { pipeline, group }
    }

    pub fn record(
        &self,
        _scope: &PassScope<'_>,
        list: &mut dyn CommandList,
    ) -> Result<(), chassis::Error> {
        list.bind_pipeline(&self.pipeline);
        list.bind_group(COMPOSITE_SET, &self.group);
        list.draw(3, 1);
        Ok(())
    }
}
