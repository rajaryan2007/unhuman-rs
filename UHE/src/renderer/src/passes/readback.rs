//! The readback pass: the composed frame copied into a CPU-readable buffer.

use chassis::graph::PassScope;
use chassis::{Buffer, CommandList, Error, Texture};

/// Copies the composed image into a readable buffer, for a caller with no
/// swapchain to present to.
///
/// A pass rather than a side channel because the graph owns the transition: the
/// copy needs the image in `TransferSrc`, and declaring the read is what emits
/// that barrier. `copy_texture_to_buffer` on its own would read whichever layout
/// the composition left behind.
pub struct ReadbackPass {
    pub source: Texture,
    pub target: Buffer,
}

impl ReadbackPass {
    pub fn record(&self, _scope: &PassScope<'_>, list: &mut dyn CommandList) -> Result<(), Error> {
        list.copy_texture_to_buffer(&self.source, &self.target);
        Ok(())
    }
}
