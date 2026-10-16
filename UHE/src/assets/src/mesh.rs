use std::sync::Arc;

use crate::image::{Image, MipLevel};

/// How a keyframe's interpolation is meant (`glTF` sampler interpolation).
///
/// `CubicSpline` carries the Hermite tangents from the glTF output layout
/// `[inTangent_i, value_i, outTangent_i]`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Interpolation {
    Step,
    #[default]
    Linear,
    CubicSpline,
}

/// One translation or scale curve, targeting a single node.
///
/// `times`/`values` are parallel vectors rather than keyframes, because that is
/// what a keep-mask wants to index anyway. `target` is the glTF node index.
///
/// `in_tangents`/`out_tangents` are parallel to `values` and empty for
/// `Step`/`Linear` tracks, so those stay compact.
#[derive(Clone, Debug, Default)]
pub struct VectorTrack {
    pub target: u32,
    pub interpolation: Interpolation,
    pub times: Vec<f32>,
    pub values: Vec<[f32; 3]>,
    pub in_tangents: Vec<[f32; 3]>,
    pub out_tangents: Vec<[f32; 3]>,
}

/// One rotation curve, targeting a single node.
///
/// Values are unit quaternions in `glTF` order `[x, y, z, w]`; `CubicSpline`
/// tangents are derivatives and are deliberately *not* normalised (glTF 2.0
/// animation section).
#[derive(Clone, Debug, Default)]
pub struct QuaternionTrack {
    pub target: u32,
    pub interpolation: Interpolation,
    pub times: Vec<f32>,
    pub values: Vec<[f32; 4]>,
    pub in_tangents: Vec<[f32; 4]>,
    pub out_tangents: Vec<[f32; 4]>,
}

/// One animation clip: three lists of curves, a name and a duration.
///
/// `duration` is the largest input time across every channel of the clip, which
/// is what playback wraps on and what the compression tolerances are stated
/// against.
#[derive(Clone, Debug, Default)]
pub struct AnimationData {
    pub name: String,
    pub duration: f32,
    pub position_tracks: Vec<VectorTrack>,
    pub rotation_tracks: Vec<QuaternionTrack>,
    pub scale_tracks: Vec<VectorTrack>,
}

/// One bone of a skin's joint hierarchy.
///
/// `id` is the index into the owning [`SkeletonData`]'s `bones`, and equals the
/// glTF node index it came from; `parent` is -1 at a root.
#[derive(Clone, Debug)]
pub struct BoneData {
    pub name: String,
    pub id: i32,
    pub parent: i32,
    pub inverse_bind: [[f32; 4]; 4],
    pub local: [[f32; 4]; 4],
}

/// A joint hierarchy with the inverse bind pose that puts a mesh in it.
///
/// One hierarchy is built across all nodes, and each skin binds a subset of it.
#[derive(Clone, Debug, Default)]
pub struct SkeletonData {
    pub bones: Vec<BoneData>,
    /// Joint node indices of the first skin, so call sites do not need a skin
    /// lookup.
    pub joints: Vec<i32>,
    pub root: i32,
}

/// One skin: the joints it binds and their inverse bind matrices.
///
/// `inverse_bind` is parallel to `joints`.
#[derive(Clone, Debug, Default)]
pub struct SkinData {
    pub name: String,
    pub joints: Vec<i32>,
    pub inverse_bind: Vec<[[f32; 4]; 4]>,
}

/// How a material's alpha is interpreted (`glTF` `alphaMode`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AlphaMode {
    /// Alpha ignored.
    #[default]
    Opaque,
    /// Fragment discarded below `alpha_cutoff`.
    Mask,
    /// Alpha blended. Needs a depth-sorted pass, which does not exist yet, so
    /// the renderer can warn instead of drawing it in submission order.
    Blend,
}

/// `glTF` sampler filter. The mip-map variant folds into these two, because
/// mip selection is the sampler's other axis, not a third filter.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Filter {
    Nearest,
    Linear,
}

/// `glTF` sampler wrap mode, per axis.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Wrap {
    Repeat,
    MirroredRepeat,
    ClampToEdge,
}

/// The sampler a texture declares, with the `glTF` spec defaults.
///
/// Mag LINEAR, min LINEAR_MIPMAP_LINEAR, REPEAT wrap. A file with no sampler
/// object gets exactly these.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SamplerData {
    pub mag_filter: Filter,
    pub min_filter: Filter,
    pub wrap_s: Wrap,
    pub wrap_t: Wrap,
}

impl Default for SamplerData {
    fn default() -> Self {
        Self {
            mag_filter: Filter::Linear,
            min_filter: Filter::Linear,
            wrap_s: Wrap::Repeat,
            wrap_t: Wrap::Repeat,
        }
    }
}

/// A decoded material texture with the sampler it was declared with.
///
/// Device-free: the pixels live on the CPU, so the material list exists without
/// a device. The pixels are shared (`Arc`), because one glTF image is routinely
/// bound by several materials and slots. `mips` is the chain the file shipped,
/// empty when it shipped none: recomputing an authored chain would discard its
/// filter.
#[derive(Clone, Debug)]
pub struct TextureData {
    pub image: Arc<Image>,
    pub mips: Arc<[MipLevel]>,
    pub sampler: SamplerData,
}

/// A material's PBR factors and the five core texture slots.
///
/// The optional `KHR_materials_*` blocks, the specular-glossiness path and
/// `KHR_texture_transform` are **not** read: the workspace enables `gltf`'s
/// `utils`/`names`/`extensions` features only, so those accessors do not exist
/// in this build. A file that declares them still loads; it renders without
/// them.
#[derive(Clone, Debug)]
pub struct MaterialData {
    pub name: String,
    /// `baseColorFactor`, spec default opaque white.
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    /// `normalTexture.scale`, spec default 1.0; a 0.0 default silently
    /// disables every normal map in the scene.
    pub normal_scale: f32,
    /// `occlusionTexture.strength`, spec default 1.0.
    pub occlusion_strength: f32,
    /// `emissiveFactor`, spec default **black**: defaulting to white makes
    /// every unlit surface glow.
    pub emissive: [f32; 3],
    pub alpha: AlphaMode,
    /// `alphaCutoff`, spec default 0.5. Only meaningful for [`AlphaMode::Mask`].
    pub alpha_cutoff: f32,
    /// Selects the cull-mode pipeline variant.
    pub double_sided: bool,
    pub albedo: Option<Arc<TextureData>>,
    pub metallic_roughness: Option<Arc<TextureData>>,
    pub normal: Option<Arc<TextureData>>,
    pub occlusion: Option<Arc<TextureData>>,
    pub emissive_texture: Option<Arc<TextureData>>,
}

impl Default for MaterialData {
    /// The material a primitive gets when the file names none, with the spec's
    /// defaults rather than `f32::default()`. A zeroed `roughness` and a
    /// `normal_scale` of 0 would both change how every unlidded surface renders.
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 1.0,
            normal_scale: 1.0,
            occlusion_strength: 1.0,
            emissive: [0.0, 0.0, 0.0],
            alpha: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
            albedo: None,
            metallic_roughness: None,
            normal: None,
            occlusion: None,
            emissive_texture: None,
        }
    }
}

/// CPU-side geometry for one `glTF` mesh primitive, plus the material and the
/// model-level animation that goes with it.
///
/// One `MeshData` per primitive, not per node: a `glTF` mesh is extracted once
/// and every node that references it points at the same entry (see
/// [`ModelNode::primitives`]), which is what keeps repeated props and foliage
/// from being duplicated per instance.
///
/// The four vertex arrays are `[f32; ...]`, so a consumer never has to know
/// about a math library to read geometry. `joints`/`weights`/`colors` are
/// additional, because a skinned mesh cannot be posed without them.
///
/// `animations` and `skeleton` are model-level facts the contract nevertheless
/// asks for on `MeshData`. They are filled on the *first* mesh of a model and
/// left empty on the rest, because cloning every clip once per primitive would
/// cost more than the geometry does; [`crate::Assets::meshes`] returns the list
/// and the first entry is the one that carries them.
#[derive(Clone, Debug, Default)]
pub struct MeshData {
    /// The `glTF` mesh name, `"Unnamed_Mesh"` when the file gave none.
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub tangents: Vec<[f32; 4]>,
    pub colors: Vec<[f32; 4]>,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    /// True when this primitive carried `COLOR_0`. One fact about the whole
    /// primitive, not per vertex: the shader multiplies vertex colour in only
    /// when it is set, and `COLOR_0` is linear data that must not be
    /// gamma-decoded.
    pub has_vertex_color: bool,
    pub indices: Vec<u32>,
    /// Axis-aligned bounds in the mesh's own space, node transforms not applied.
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub material: Option<MaterialData>,
    pub animations: Vec<AnimationData>,
    pub skeleton: Option<SkeletonData>,
}

/// One node of the `glTF` scene graph, flattened.
///
/// The editor turns each node into a controllable child entity instead of one
/// opaque model.
#[derive(Clone, Debug)]
pub struct ModelNode {
    pub name: String,
    /// Index into [`ModelData::nodes`], -1 at a root.
    pub parent: i32,
    pub children: Vec<u32>,
    /// The `glTF` mesh index this node draws, -1 when it draws nothing.
    pub mesh: i32,
    /// The entries of the model's mesh list this node instantiates, in
    /// primitive order.
    pub primitives: Vec<u32>,
    pub translation: [f32; 3],
    /// `[x, y, z, w]`, `glTF` order.
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
    /// Every ancestor's transform composed down to and including this one.
    ///
    /// Deliberately not baked into the vertices: a skinning bone matrix has to
    /// apply on top of it, and rewriting every vertex per instance would defeat
    /// the geometry sharing.
    pub world_transform: [[f32; 4]; 4],
}

/// Everything about a loaded model that is not per-primitive geometry.
///
/// The pieces the contract does not put on [`MeshData`] live here so a caller
/// can still reach the node graph, the skins and the list of extensions the
/// loader did not implement.
#[derive(Clone, Debug, Default)]
pub struct ModelData {
    pub nodes: Vec<ModelNode>,
    /// Root node indices, in scene order.
    pub roots: Vec<u32>,
    pub skins: Vec<SkinData>,
    /// Extensions the file declares that this loader does not honour, by name.
    ///
    /// Reported at load time because a parsed-but-ignored extension makes an
    /// asset render with subtly wrong shading and no error anywhere.
    pub unsupported_extensions: Vec<String>,
}
