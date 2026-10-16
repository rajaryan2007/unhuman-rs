//! Framebuffers.
//!
//! ## Status
//!
//! No code. `chassis::Gpu` renders with dynamic rendering on Vulkan and binds the
//! attachments directly to the pass on wgpu, so neither backend needs a
//! framebuffer object: both use dynamic rendering / implicit attachments and have
//! no equivalent.
