// The depth prepass and the shadow map: one vertex stage, no colour output.
//
// `ShaderSet::fragment_for` returns `None` for the depth variant, so this module
// has a vertex entry point and nothing else. A prepass is cheaper than writing
// depth from the model shader, and is what the shadow map needs.

struct Shadow {
    light_view_projection: mat4x4<f32>,
};

struct Draw {
    model: mat4x4<f32>,
    tint: vec4<f32>,
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> shadow: Shadow;
@group(1) @binding(0) var<uniform> draw: Draw;

@vertex
fn vs_main(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return shadow.light_view_projection * draw.model * vec4<f32>(position, 1.0);
}
