//! Named, bounded procedural material programs.
use crate::{Fail, fail};
use ve_graph::{Node, Value as Param};

struct Preset {
    color: [f32; 3],
    metallic: f32,
    roughness: f32,
    noise_x: u32,
    noise_y: u32,
    colors: [&'static str; 3],
    pattern: Option<&'static str>,
}

fn data(name: &str) -> Option<Preset> {
    let preset = match name {
        "oak" => Preset {
            color: [0.42, 0.25, 0.12],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 2,
            noise_y: 30,
            colors: ["#382416", "#75502e", "#b88b52"],
            pattern: None,
        },
        "planks" => Preset {
            color: [0.48, 0.34, 0.20],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 2,
            noise_y: 28,
            colors: ["#49311e", "#8b6842", "#c4a276"],
            pattern: Some("bricks(rows=1, columns=7, row_offset=0, mortar=0.035, bevel=0.02)"),
        },
        "red_brick" => Preset {
            color: [0.55, 0.20, 0.12],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 8,
            noise_y: 8,
            colors: ["#582b20", "#a44b35", "#d47755"],
            pattern: Some("bricks(rows=8, columns=4, row_offset=0.5, mortar=0.08, bevel=0.2)"),
        },
        "cobblestone" => Preset {
            color: [0.42, 0.40, 0.35],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 8,
            noise_y: 8,
            colors: ["#48463e", "#898477", "#b9b09d"],
            pattern: Some("bricks(rows=6, columns=4, row_offset=0.5, mortar=0.12, bevel=0.35)"),
        },
        "concrete" => Preset {
            color: [0.48, 0.48, 0.46],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 12,
            noise_y: 12,
            colors: ["#686967", "#a5a6a2", "#d0d0ca"],
            pattern: None,
        },
        "plaster" => Preset {
            color: [0.68, 0.62, 0.52],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 5,
            noise_y: 5,
            colors: ["#8d7962", "#c2b197", "#e7dbc2"],
            pattern: None,
        },
        "rusted_iron" => Preset {
            color: [0.42, 0.25, 0.16],
            metallic: 0.75,
            roughness: 1.0,
            noise_x: 9,
            noise_y: 9,
            colors: ["#42281c", "#98502e", "#c07845"],
            pattern: None,
        },
        "brushed_steel" => Preset {
            color: [0.52, 0.56, 0.59],
            metallic: 0.9,
            roughness: 1.0,
            noise_x: 2,
            noise_y: 36,
            colors: ["#69747a", "#aab4b8", "#e0e4e4"],
            pattern: None,
        },
        "leather" => Preset {
            color: [0.30, 0.16, 0.09],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 10,
            noise_y: 10,
            colors: ["#382219", "#74452d", "#a8754d"],
            pattern: None,
        },
        "linen" => Preset {
            color: [0.68, 0.60, 0.44],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 26,
            noise_y: 26,
            colors: ["#887b5f", "#c0b18e", "#e7d9b8"],
            pattern: Some("weave(columns=36, rows=36, width=0.82)"),
        },
        "sand" => Preset {
            color: [0.69, 0.52, 0.31],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 22,
            noise_y: 22,
            colors: ["#8e6940", "#c4a06e", "#e2c895"],
            pattern: None,
        },
        "mud" => Preset {
            color: [0.30, 0.20, 0.12],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 8,
            noise_y: 8,
            colors: ["#382719", "#705033", "#a17b50"],
            pattern: None,
        },
        "snow" => Preset {
            color: [0.82, 0.87, 0.91],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 5,
            noise_y: 5,
            colors: ["#aab9c4", "#e0e9ee", "#ffffff"],
            pattern: None,
        },
        "ice" => Preset {
            color: [0.38, 0.67, 0.78],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 5,
            noise_y: 5,
            colors: ["#477b91", "#8fc5d4", "#d4f3f5"],
            pattern: None,
        },
        "asphalt" => Preset {
            color: [0.10, 0.10, 0.11],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 14,
            noise_y: 14,
            colors: ["#101012", "#2a2b2e", "#4a4b4f"],
            pattern: None,
        },
        "corrugated_metal" => Preset {
            color: [0.40, 0.42, 0.45],
            metallic: 0.8,
            roughness: 1.0,
            noise_x: 3,
            noise_y: 24,
            colors: ["#3a3f45", "#6a7077", "#aab0b6"],
            pattern: Some("bricks(rows=1, columns=16, row_offset=0, mortar=0.3, bevel=0.9)"),
        },
        "painted_metal" => Preset {
            color: [0.07, 0.12, 0.2],
            metallic: 0.6,
            roughness: 1.0,
            noise_x: 6,
            noise_y: 6,
            colors: ["#0b1422", "#18304f", "#2c5a86"],
            pattern: None,
        },
        "wall_tile" => Preset {
            color: [0.62, 0.7, 0.7],
            metallic: 0.0,
            roughness: 1.0,
            noise_x: 6,
            noise_y: 6,
            colors: ["#5a6c6c", "#9bb0ae", "#d9e6e2"],
            pattern: Some("bricks(rows=14, columns=14, row_offset=0, mortar=0.1, bevel=0.3)"),
        },
        _ => return None,
    };
    Some(preset)
}

fn number(node: &Node, key: &str, default: f64) -> Result<f64, Fail> {
    // A whole number is a number: the saved document writes a slider at 0 or 1 as an integer.
    match node.params.get(key).map(Param::as_f64) {
        Some(Some(v)) if v.is_finite() && (0.0..=1.0).contains(&v) => Ok(v),
        None => Ok(default),
        _ => Err(fail(
            "param.range",
            format!("{key} must be between 0 and 1"),
        )),
    }
}

fn f(value: f64) -> String {
    format!("{value:.3}")
}

/// Returns the PBR base values and a DSL program for the selected preset.
pub(crate) fn build(node: &Node) -> Result<([f32; 3], f32, f32, String), Fail> {
    let name = match node.params.get("name") {
        Some(Param::Str(name)) => name.as_str(),
        None => "oak",
        _ => return Err(fail("param.type", "material.preset name must be a choice")),
    };
    let preset = data(name)
        .ok_or_else(|| fail("param.range", format!("unknown material preset `{name}`")))?;
    let wear = number(node, "wear", 0.3)?;
    let dirt = number(node, "dirt", 0.2)?;
    let moss = number(node, "moss", 0.0)?;
    let wet = number(node, "wet", 0.0)?;
    let mut statements = vec![format!(
        "n = fbm(scale_x={}, scale_y={}, iterations=5)",
        preset.noise_x, preset.noise_y
    )];
    if let Some(pattern) = preset.pattern {
        statements.push(format!("p = {pattern}"));
    }
    statements.push(format!(
        "base = colorize(n, gradient=[0:{}, 0.5:{}, 1:{}])",
        preset.colors[0], preset.colors[1], preset.colors[2]
    ));
    let mut albedo = "base".to_owned();
    // Every input of a texture node is a node defined above, never an inline call.
    statements.push("rough0 = colorize(n, gradient=[0:#c8c8c8, 1:#f0f0f0])".to_owned());
    let mut rough = "rough0".to_owned();
    let mut height = "n".to_owned();
    if preset.pattern.is_some() {
        statements
            .push("pat = colorize(p, gradient=[0:#383838, 0.08:#ffffff, 1:#ffffff])".to_owned());
        statements
            .push("patterned = blend(pat, base, pat, blend_type=multiply, amount=1.0)".to_owned());
        albedo = "patterned".to_owned();
        height = "p".to_owned();
    }
    if wear > 0.0 {
        statements.push(format!("edge = surface_convex(gain={})", f(wear * 2.0)));
        statements.push("worn = colorize(edge, gradient=[0:#ffffff, 1:#e8ddc8])".to_owned());
        statements.push(format!(
            "wear_albedo = blend(worn, {albedo}, edge, blend_type=normal, amount={})",
            f(wear)
        ));
        albedo = "wear_albedo".to_owned();
        statements.push("edge_rough = colorize(edge, gradient=[0:#d0d0d0, 1:#ffffff])".to_owned());
        statements.push(format!(
            "wear_rough = blend(edge_rough, {rough}, edge, blend_type=normal, amount={})",
            f(wear)
        ));
        rough = "wear_rough".to_owned();
    }
    if dirt > 0.0 {
        statements.push(format!("cavity = surface_cavity(gain={})", f(dirt * 3.0)));
        statements.push(format!(
            "dirt_albedo = blend(cavity, {albedo}, cavity, blend_type=multiply, amount={})",
            f(dirt)
        ));
        albedo = "dirt_albedo".to_owned();
    }
    if moss > 0.0 {
        statements.push("up = surface_up(power=2)".to_owned());
        statements.push("moss_color = colorize(n, gradient=[0:#30452b, 1:#73934b])".to_owned());
        statements.push(format!(
            "moss_albedo = blend(moss_color, {albedo}, up, blend_type=normal, amount={})",
            f(moss)
        ));
        albedo = "moss_albedo".to_owned();
        statements.push(format!(
            "moss_rough = blend(moss_color, {rough}, up, blend_type=normal, amount={})",
            f(moss)
        ));
        rough = "moss_rough".to_owned();
        statements.push(format!(
            "moss_height = blend(up, {height}, up, blend_type=normal, amount={})",
            f(moss * 0.2)
        ));
        height = "moss_height".to_owned();
    }
    if wet > 0.0 {
        statements.push(format!(
            "wet_albedo = blend(base, {albedo}, n, blend_type=multiply, amount={})",
            f(wet * 0.18)
        ));
        albedo = "wet_albedo".to_owned();
        statements.push(format!(
            "wet_rough = blend(n, {rough}, n, blend_type=multiply, amount={})",
            f(wet * 0.65)
        ));
        rough = "wet_rough".to_owned();
    }
    statements.push(format!("albedo = {albedo}"));
    statements.push(format!("roughness = {rough}"));
    statements.push(format!("height = {height}"));
    if preset.metallic > 0.0 {
        statements.push("metallic = colorize(n, gradient=[0:#b0b0b0, 1:#ffffff])".to_owned());
    }
    let program = statements.join("; ");
    if program.len() > 8192 {
        return Err(fail(
            "param.range",
            "material preset program exceeds 8,192 bytes",
        ));
    }
    Ok((preset.color, preset.metallic, preset.roughness, program))
}
