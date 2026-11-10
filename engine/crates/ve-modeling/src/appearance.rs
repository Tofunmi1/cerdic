//! `solid.material`: the shape passes through unchanged and carries a surface.
use crate::{Fail, Material, MaterialRecipe, cache::Value, fail, instances::InstanceSet};
use std::rc::Rc;
use ve_graph::{Node, Value as Param};

fn triple(node: &Node, name: &str) -> Option<[f32; 3]> {
    let Some(Param::List(items)) = node.params.get(name) else {
        return None;
    };
    let mut out = [0.0_f32; 3];
    for (slot, item) in out.iter_mut().zip(items) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "colour channels are 0..=100"
        )]
        {
            *slot = item.as_f64()? as f32;
        }
    }
    (items.len() == 3).then_some(out)
}

fn scalar(node: &Node, name: &str) -> Option<f32> {
    #[expect(clippy::cast_possible_truncation, reason = "factors are 0..=1")]
    node.params.get(name)?.as_f64().map(|v| v as f32)
}

pub(crate) fn material(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "solid.material needs a solid"))?;
    let (Some(color), Some(metallic), Some(roughness), Some(emissive)) = (
        triple(node, "color"),
        scalar(node, "metallic"),
        scalar(node, "roughness"),
        triple(node, "emissive"),
    ) else {
        return Err(fail(
            "kernel.invalid_input",
            "solid.material needs color, metallic, roughness and emissive",
        ));
    };
    let recipe = match node.params.get("recipe") {
        None => MaterialRecipe::None,
        Some(Param::Str(value)) if value == "none" => MaterialRecipe::None,
        Some(Param::Str(value)) if value == "wood" => MaterialRecipe::Wood,
        Some(Param::Str(value)) if value == "planks" => MaterialRecipe::Planks,
        Some(Param::Str(value)) if value == "brushed_metal" => MaterialRecipe::BrushedMetal,
        Some(Param::Str(value)) if value == "leather" => MaterialRecipe::Leather,
        Some(Param::Str(value)) if value == "fabric" => MaterialRecipe::Fabric,
        Some(Param::Str(value)) if value == "brick" => MaterialRecipe::Brick,
        Some(Param::Str(value)) if value == "concrete" => MaterialRecipe::Concrete,
        Some(Param::Str(value)) if value == "marble" => MaterialRecipe::Marble,
        Some(Param::Str(value)) if value == "painted_wall" => MaterialRecipe::PaintedWall,
        Some(Param::Str(value)) if value == "water" => MaterialRecipe::Water,
        _ => return Err(fail("param.range", "unknown procedural material recipe")),
    };
    let texture_scale = scalar(node, "texture_scale").unwrap_or(1.0);
    let opacity = scalar(node, "opacity").unwrap_or(1.0);
    let surface = Material {
        color,
        opacity,
        metallic,
        roughness,
        emissive,
        emits_light: matches!(node.params.get("emits_light"), Some(Param::Bool(true))),
        recipe,
        texture_scale,
        texture_tile: 0.0,
        no_repeat: false,
        no_repeat_scale: 1.0,
        texture_size: 0,
        toon: None,
        texture_program: None,
        texture_style: None,
        foliage: None,
        finish: None,
    };
    // A placement set keeps sharing one prototype; the surface goes on that prototype, so every
    // placement draws with it and the set stays a set.
    let instances = input.instances.as_ref().map(|set| {
        let mut prototype = (*set.prototype).clone();
        prototype.material = Some(surface.clone());
        prototype.regions.clear();
        InstanceSet {
            prototype: Rc::new(prototype),
            transforms: set.transforms.clone(),
        }
    });
    Ok(Value {
        mesh: input.mesh.clone(),
        uvs: input.uvs.clone(),
        volume: input.volume,
        loops: Vec::new(),
        curve: None,
        material: Some(surface),
        // A new surface replaces every colour the input had.
        regions: Vec::new(),
        instances,
        frames: input.frames.clone(),
        poly: input.poly.clone(),
        selection: None,
        field: input.field.clone(),
        warnings: input.warnings.clone(),
        light: None,
        environment: None,
        animation: input.animation.clone(),
        state_machine: input.state_machine.clone(),
        state_machine_node: input.state_machine_node.clone(),
        behavior: input.behavior.clone(),
        parent: input.parent.clone(),
        bone: input.bone,
        skin: input.skin.clone(),
        morph_targets: input.morph_targets.clone(),
        ik: input.ik.clone(),
        fx: input.fx.clone(),
        ragdoll: input.ragdoll.clone(),
        body: input.body.clone(),
        number: None,
        pattern: None,
        game: None,
        baked: input.baked.clone(),
        human: input.human.clone(),
        human_source_indices: input.human_source_indices.clone(),
        poly_human_source_indices: input.poly_human_source_indices.clone(),
        grid: input.grid.clone(),
        colors: input.colors.clone(),
        visible_range: input.visible_range,
        visible_fade: input.visible_fade,
        sdf: None,
        // A surface put on a garment keeps it cloth.
        cloth: input.cloth.clone(),
        fluid: input.fluid.clone(),
        sim: input.sim.clone(),
    })
}

/// Attaches a bounded procedural texture program without changing the base PBR surface.
pub(crate) fn texture(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "solid.texture needs a solid"))?;
    let Some(Param::Str(program)) = node.params.get("program") else {
        return Err(fail("param.type", "solid.texture needs a program string"));
    };
    if program.trim().is_empty() {
        return Err(fail("param.range", "solid.texture program cannot be empty"));
    }
    let tile = scalar(node, "tile").unwrap_or(0.0).max(0.0);
    let size = match node.params.get("size") {
        Some(Param::Str(word)) if word != "auto" => match word.parse::<u32>() {
            Ok(side @ (256 | 512 | 1024)) => side,
            _ => return Err(fail("param.range", "size must be auto, 256, 512 or 1024")),
        },
        _ => 0,
    };
    let no_repeat = matches!(node.params.get("no_repeat"), Some(Param::Bool(true)));
    let no_repeat_scale = scalar(node, "no_repeat_scale").unwrap_or(1.0);
    if !(0.25..=8.0).contains(&no_repeat_scale) {
        return Err(fail(
            "param.range",
            "no_repeat_scale must be between 0.25 and 8",
        ));
    }
    Ok(with_surfaces(input, |existing| {
        let mut material = textured_surface(existing, program, tile, size);
        material.no_repeat = no_repeat;
        material.no_repeat_scale = no_repeat_scale;
        material
    }))
}

/// Applies a named procedural preset through the same textured surface path as `solid.texture`.
pub(crate) fn preset(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "material.preset needs a solid"))?;
    let (color, metallic, roughness, program) = crate::material_presets::build(node)?;
    let tile = scalar(node, "tile").unwrap_or(1.0);
    if !(0.1..=10.0).contains(&tile) {
        return Err(fail(
            "param.range",
            "material.preset tile must be between 0.1 and 10 metres",
        ));
    }
    let size = match node.params.get("size") {
        Some(Param::Str(word)) => word
            .parse::<u32>()
            .ok()
            .filter(|n| matches!(n, 256 | 512 | 1024)),
        None => Some(512),
        _ => None,
    }
    .ok_or_else(|| {
        fail(
            "param.range",
            "material.preset size must be 256, 512 or 1024",
        )
    })?;
    let mut value = (**input).clone();
    let apply = |existing: Option<&Material>| {
        let mut material = textured_surface(existing, &program, tile, size);
        material.color = color;
        material.metallic = metallic;
        material.roughness = roughness;
        material
    };
    value.material = Some(apply(value.material.as_ref()));
    for region in &mut value.regions {
        region.material = Some(apply(region.material.as_ref()));
    }
    if let Some(set) = &input.instances {
        let mut prototype = (*set.prototype).clone();
        prototype.material = Some(apply(prototype.material.as_ref()));
        for region in &mut prototype.regions {
            region.material = Some(apply(region.material.as_ref()));
        }
        value.instances = Some(InstanceSet {
            prototype: Rc::new(prototype),
            transforms: set.transforms.clone(),
        });
    }
    Ok(value)
}

fn textured_surface(existing: Option<&Material>, program: &str, tile: f32, size: u32) -> Material {
    let mut material = existing.cloned().unwrap_or(Material {
        color: [0.8; 3],
        opacity: 1.0,
        metallic: 0.0,
        // The baked roughness map is multiplied by this factor, so a program that paints roughness gets it at full strength
        // (0.5 would halve every value and make sand or bark look wet); without a roughness channel the old 0.5 stands.
        roughness: if program.contains("roughness") {
            1.0
        } else {
            0.5
        },
        emissive: [0.0; 3],
        emits_light: false,
        recipe: MaterialRecipe::None,
        texture_scale: 1.0,
        texture_tile: 0.0,
        no_repeat: false,
        no_repeat_scale: 1.0,
        texture_size: 0,
        toon: None,
        texture_program: None,
        texture_style: None,
        foliage: None,
        finish: None,
    });
    material.texture_program = Some(program.to_owned());
    material.texture_tile = tile;
    material.texture_size = size;
    material
}

/// Gives a surface cel shading without changing its colour, roughness or textures. A bare shape gets a default material.
pub(crate) fn toon(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "solid.toon needs a solid"))?;
    let bands = match node.params.get("bands") {
        Some(Param::Int(n)) if (1..=4).contains(n) => u32::try_from(*n).unwrap_or(2),
        None => 2,
        _ => return Err(fail("param.range", "solid.toon bands must be 1 to 4")),
    };
    let shade_color = triple(node, "shade_color").unwrap_or([0.62, 0.58, 0.82]);
    let toon = crate::result::Toon {
        bands,
        feather: scalar(node, "feather").unwrap_or(0.08).clamp(0.0, 1.0),
        shift: scalar(node, "shift").unwrap_or(0.0).clamp(-1.0, 1.0),
        spec: scalar(node, "spec").unwrap_or(0.0).clamp(0.0, 1.0),
        shade_color,
        rim: scalar(node, "rim").unwrap_or(0.0).clamp(0.0, 1.0),
        outline: scalar(node, "outline").unwrap_or(0.0).clamp(0.0, 16.0),
        outline_color: triple(node, "outline_color").unwrap_or([0.1, 0.06, 0.14]),
    };
    Ok(restyle(input, Some(toon), None))
}

/// `input` with its surface (and every region's, and a placement set's prototype's) set to cel shading `toon` (or back to
/// physically based shading when `None`), and its texture maps finished the way `texture_style` says. Colours, roughness and
/// texture programs are kept.
pub(crate) fn restyle(
    input: &Value,
    toon: Option<crate::result::Toon>,
    texture_style: Option<&str>,
) -> Value {
    with_surfaces(input, |existing| {
        let mut material = textured_surface_default(existing);
        material.toon.clone_from(&toon);
        material.texture_style = texture_style.map(str::to_owned);
        material
    })
}

/// `input` with `apply` run on its surface, on every region's, and on a placement set's prototype (and its regions), each given
/// the surface already there, if any.
pub(crate) fn with_surfaces(input: &Value, apply: impl Fn(Option<&Material>) -> Material) -> Value {
    let mut value = input.clone();
    value.material = Some(apply(value.material.as_ref()));
    for region in &mut value.regions {
        region.material = Some(apply(region.material.as_ref()));
    }
    if let Some(set) = &input.instances {
        let mut prototype = (*set.prototype).clone();
        prototype.material = Some(apply(prototype.material.as_ref()));
        for region in &mut prototype.regions {
            region.material = Some(apply(region.material.as_ref()));
        }
        value.instances = Some(InstanceSet {
            prototype: Rc::new(prototype),
            transforms: set.transforms.clone(),
        });
    }
    value
}

/// The surface already on a shape, or the default one.
pub(crate) fn textured_surface_default(existing: Option<&Material>) -> Material {
    existing.cloned().unwrap_or(Material {
        color: [0.8; 3],
        opacity: 1.0,
        metallic: 0.0,
        roughness: 0.5,
        emissive: [0.0; 3],
        emits_light: false,
        recipe: MaterialRecipe::None,
        texture_scale: 1.0,
        texture_tile: 0.0,
        no_repeat: false,
        no_repeat_scale: 1.0,
        texture_size: 0,
        toon: None,
        texture_program: None,
        texture_style: None,
        foliage: None,
        finish: None,
    })
}
