//! Cel ("toon") shading and outlines on Bevy's renderer: the look of the anime style and of the generated characters. A surface with toon
//! bands gets [`ToonMaterial`], Bevy's standard material extended with `shaders/toon.wesl` (flat light bands, tinted shadow, rim,
//! highlight). A surface with an outline width also gets a child entity that draws the same mesh again as a back-face hull pushed out
//! along its smooth normal by a fixed number of screen pixels (`shaders/outline.wesl`).

use bevy_app::{App, Plugin, Update};
use bevy_asset::{Asset, Assets, Handle, embedded_asset};
use bevy_camera::visibility::Visibility;
use bevy_color::LinearRgba;
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use bevy_math::Vec4;
use bevy_mesh::skinning::SkinnedMesh;
use bevy_mesh::{Mesh, Mesh3d, MeshVertexBufferLayoutRef};
use bevy_pbr::{
    ExtendedMaterial, Material, MaterialExtension, MaterialPipeline, MaterialPipelineKey,
    MaterialPlugin, MeshMaterial3d, StandardMaterial,
};
use bevy_reflect::TypePath;
use bevy_render::render_resource::{
    AsBindGroup, Face, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy_shader::ShaderRef;
use bevy_transform::components::Transform;
use ve_render::{ToonStyle, VeMaterial};

/// The toon parameters the shader reads: `a` is bands, feather, shift and highlight size; `b` the shadow colour multiplier and rim.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct ToonExtension {
    #[uniform(100)]
    a: Vec4,
    #[uniform(100)]
    b: Vec4,
}

impl MaterialExtension for ToonExtension {
    fn fragment_shader() -> ShaderRef {
        "embedded://ve_bevy/shaders/toon.wesl".into()
    }
}

/// The standard material with cel lighting.
pub type ToonMaterial = ExtendedMaterial<StandardMaterial, ToonExtension>;

/// The shader parameters of a toon style.
#[must_use]
pub fn toon_extension(style: &ToonStyle) -> ToonExtension {
    let shade = LinearRgba::from(style.shade_color);
    #[expect(clippy::cast_precision_loss, reason = "bands are at most four")]
    let bands = style.bands.min(4) as f32;
    ToonExtension {
        a: Vec4::new(
            bands,
            style.feather.clamp(0.0, 1.0),
            style.shift.clamp(-1.0, 1.0),
            style.spec.clamp(0.0, 1.0),
        ),
        b: Vec4::new(
            shade.red,
            shade.green,
            shade.blue,
            style.rim.clamp(0.0, 1.0),
        ),
    }
}

/// Procedural pixel-shader relief (`shaders/detail.wesl`): wrinkles, scales or pores finer than the mesh.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct DetailExtension {
    #[uniform(100)]
    a: Vec4,
    #[uniform(100)]
    b: Vec4,
    /// x thickness (sunlight through a thin body).
    #[uniform(100)]
    c: Vec4,
}

impl MaterialExtension for DetailExtension {
    fn fragment_shader() -> ShaderRef {
        "embedded://ve_bevy/shaders/detail.wesl".into()
    }
}

/// The standard material with procedural relief.
pub type DetailMaterial = ExtendedMaterial<StandardMaterial, DetailExtension>;

/// The shader parameters of a surface finish with relief.
#[must_use]
pub fn detail_extension(finish: &ve_render::SurfaceFinish) -> DetailExtension {
    DetailExtension {
        a: Vec4::new(finish.relief, finish.relief_scale, finish.relief_style, 3.0),
        b: Vec4::new(
            finish.wetness,
            finish.iridescence,
            finish.sheen,
            finish.subsurface,
        ),
        c: Vec4::new(finish.thickness, 0.0, 0.0, 0.0),
    }
}

/// Whether a finish needs the detail shader (relief, wetness, iridescence, sheen or subsurface scattering).
#[must_use]
pub fn needs_detail(finish: &ve_render::SurfaceFinish) -> bool {
    finish.relief > 0.0
        || finish.wetness > 0.0
        || finish.iridescence > 0.0
        || finish.sheen > 0.0
        || finish.subsurface > 0.0
        || finish.thickness > 0.0
}

/// The back-face hull of a toon outline: its colour (linear) and width in screen pixels.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone)]
pub struct OutlineMaterial {
    #[uniform(0)]
    color: Vec4,
}

impl Material for OutlineMaterial {
    fn vertex_shader() -> ShaderRef {
        "embedded://ve_bevy/shaders/outline.wesl".into()
    }

    fn fragment_shader() -> ShaderRef {
        "embedded://ve_bevy/shaders/outline.wesl".into()
    }

    fn enable_prepass() -> bool {
        false
    }

    fn enable_shadows() -> bool {
        false
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Position, the face normal, and the packed smooth normal that rides in the occlusion slot.
        // A skinned mesh carries a welded smooth normal that follows its deformation; without it the face normal tears the hull open.
        // A GPU skinned mesh also has joints and weights, which Bevy's mesh vertex reads at locations 6 and 7 (and the pipeline already
        // carries the skin bind group and the SKINNED shader define for it).
        let skinned = layout.0.contains(Mesh::ATTRIBUTE_JOINT_INDEX)
            && layout.0.contains(Mesh::ATTRIBUTE_JOINT_WEIGHT);
        let attributes = |normal: bevy_mesh::MeshVertexAttribute| {
            let mut wanted = vec![
                Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
                normal.at_shader_location(1),
                ve_render::ATTRIBUTE_OCCLUSION.at_shader_location(2),
            ];
            if skinned {
                wanted.push(Mesh::ATTRIBUTE_JOINT_INDEX.at_shader_location(6));
                wanted.push(Mesh::ATTRIBUTE_JOINT_WEIGHT.at_shader_location(7));
            }
            wanted
        };
        let buffer = layout
            .0
            .get_layout(&attributes(ve_render::ATTRIBUTE_HULL_NORMAL))
            .or_else(|_| layout.0.get_layout(&attributes(Mesh::ATTRIBUTE_NORMAL)))?;
        descriptor.vertex.buffers = vec![buffer];
        descriptor.primitive.cull_mode = Some(Face::Front);
        Ok(())
    }
}

/// The outline hull drawn for a surface.
#[derive(Component)]
struct OutlineHull;

/// Where a surface's outline hull is, so it can be replaced.
#[derive(Component)]
struct OutlineLink(Entity);

/// Adds the toon material and the outline hulls.
#[derive(Debug, Default, Clone, Copy)]
pub struct VeBevyToonPlugin;

impl Plugin for VeBevyToonPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/toon.wesl");
        embedded_asset!(app, "shaders/outline.wesl");
        embedded_asset!(app, "shaders/detail.wesl");
        app.add_plugins((
            MaterialPlugin::<ToonMaterial>::default(),
            MaterialPlugin::<OutlineMaterial>::default(),
            MaterialPlugin::<DetailMaterial>::default(),
        ))
        .add_systems(Update, (map_outlines, hull_follows_skin));
    }
}

/// A hull shares its surface's mesh, so once that mesh is GPU skinned the hull must be skinned by the same joints: Bevy builds a skinned
/// pipeline for any entity whose mesh has joint attributes and fails the draw when the entity has no skin.
fn hull_follows_skin(
    mut commands: Commands,
    hulls: Query<(Entity, &ChildOf), (With<OutlineHull>, Without<SkinnedMesh>)>,
    parents: Query<&SkinnedMesh>,
) {
    for (hull, parent) in &hulls {
        if let Ok(skin) = parents.get(parent.parent()) {
            commands
                .entity(hull)
                .try_insert((skin.clone(), Visibility::Inherited));
        }
    }
}

/// Gives every surface with an outline width a hull child, and takes the hull away when the width goes to zero or the surface changes.
fn map_outlines(
    mut commands: Commands,
    mut outlines: ResMut<Assets<OutlineMaterial>>,
    meshes: Res<Assets<Mesh>>,
    changed: Query<
        (Entity, &VeMaterial, &Mesh3d, Option<&OutlineLink>),
        Or<(Changed<VeMaterial>, Changed<Mesh3d>)>,
    >,
) {
    for (entity, surface, mesh, link) in &changed {
        if let Some(OutlineLink(hull)) = link {
            commands.entity(*hull).try_despawn();
            commands.entity(entity).try_remove::<OutlineLink>();
        }
        let width = surface.toon.outline_width.clamp(0.0, 16.0);
        if width <= 0.0 {
            continue;
        }
        let line = LinearRgba::from(surface.toon.outline_color);
        let material: Handle<OutlineMaterial> = outlines.add(OutlineMaterial {
            color: Vec4::new(line.red, line.green, line.blue, width),
        });
        // A hull on a GPU skinned mesh stays hidden until it has the skin too (see `hull_follows_skin`).
        let skinned = meshes
            .get(&mesh.0)
            .is_some_and(|m| m.contains_attribute(Mesh::ATTRIBUTE_JOINT_INDEX));
        let hull = commands
            .spawn((
                OutlineHull,
                if skinned {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                },
                Mesh3d(mesh.0.clone()),
                MeshMaterial3d(material),
                Transform::IDENTITY,
                bevy_camera::visibility::NoFrustumCulling,
                bevy_ecs::hierarchy::ChildOf(entity),
            ))
            .id();
        commands.entity(entity).try_insert(OutlineLink(hull));
    }
}
