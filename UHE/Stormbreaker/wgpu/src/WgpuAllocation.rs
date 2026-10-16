//! Sub-allocation inside a wgpu buffer.
//!
//! ## Status
//!
//! No code. Vulkan has an allocator because it allocates its own memory; wgpu's
//! `wgpu::Buffer` already owns its allocation and gives no way to alias one.
//!
//! The absence is deliberate, not an omission. It is listed because a reader
//! coming from the Vulkan backend will look for it, and the honest answer is
//! "wgpu does not expose the primitive this would be built from" -- not silence.
