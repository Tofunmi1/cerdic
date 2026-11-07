//! `city.ground` and `city.buildings`: a modern district laid out by `ve_world::city` and built by `ve_world::building`.
//!
//! Both nodes lay out the same streets and lots from the same parameters (the layout is a pure function and cheap), so a document builds the road, the pavement and each
//! part and style of the buildings as separate nodes with their own materials, and they all line up.
use ve_graph::{Node, Value as Param};
use ve_kernel::Mesh;
use ve_world::building::{HeroAssetParams, Mesh as Strips, build, build_hero_asset, inset};
use ve_world::city::{CityParams, Layout, layout};

mod shape;
use crate::world_terrain::param;
use crate::{Fail, cache::Value, fail};
use shape::{Shape, lot_buildings, style_of};

pub(crate) fn run(node: &Node) -> Option<Result<Value, Fail>> {
    match node.op.family().as_str() {
        "city.ground" => Some(ground(node)),
        "city.buildings" => Some(buildings(node)),
        "building.asset" => Some(building_asset(node)),
        "street.road" => Some(street_road(node)),
        "street.prop" => Some(street_prop(node)),
        _ => None,
    }
}

fn pair(node: &Node, name: &str) -> Result<[f32; 2], Fail> {
    match node.params.get(name) {
        Some(Param::List(items)) if items.len() == 2 => {
            match (items[0].as_f64(), items[1].as_f64()) {
                #[expect(clippy::cast_possible_truncation, reason = "metres")]
                (Some(x), Some(z)) => Ok([x as f32, z as f32]),
                _ => Err(fail("param.type", format!("{name} must be two numbers"))),
            }
        }
        _ => Err(fail("param.type", format!("{name} must be [x, z]"))),
    }
}

fn triple(node: &Node, name: &str) -> Result<[f32; 3], Fail> {
    match node.params.get(name) {
        Some(Param::List(items)) if items.len() == 3 => {
            match (items[0].as_f64(), items[1].as_f64(), items[2].as_f64()) {
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "bounded positions in metres"
                )]
                (Some(x), Some(y), Some(z)) => Ok([x as f32, y as f32, z as f32]),
                _ => Err(fail(
                    "param.type",
                    format!("{name} must contain three numbers"),
                )),
            }
        }
        _ => Err(fail("param.type", format!("{name} must be [x, y, z]"))),
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "metres and ratios fit f32; the seed is a whole number up to a million"
)]
#[expect(clippy::cast_sign_loss, reason = "the seed is not negative")]
fn city_params(node: &Node) -> Result<CityParams, Fail> {
    Ok(CityParams {
        size: pair(node, "size")?,
        centre: pair(node, "centre")?,
        block: param(node, "block")? as f32,
        block_depth: param(node, "block_depth")? as f32,
        street_width: param(node, "street_width")? as f32,
        lot_min: param(node, "lot_min")? as f32,
        lot_max: param(node, "lot_max")? as f32,
        irregularity: param(node, "irregularity")? as f32,
        seed: param(node, "seed")?.max(0.0) as u64,
    })
}

fn lay_out(node: &Node) -> Result<(CityParams, Layout), Fail> {
    let params = city_params(node)?;
    let layout = layout(&params)
        .map_err(|e| fail("param.range", format!("{}: {e}", node.op.family().as_str())))?;
    Ok((params, layout))
}

fn text<'a>(node: &'a Node, name: &str) -> &'a str {
    match node.params.get(name) {
        Some(Param::Str(value)) => value.as_str(),
        _ => "",
    }
}

/// The most triangles the fire escapes of a district can have: every building from 18 to 32 m, seven boxes (84 triangles) per upper floor.
#[expect(clippy::cast_possible_truncation, reason = "storey counts are small")]
#[expect(clippy::cast_sign_loss, reason = "storey counts are not negative")]
fn fire_escape_triangles(node: &Node, layout: &Layout, detail: f64) -> usize {
    if detail < 0.34 {
        return 0;
    }
    let scale = param(node, "height_scale").unwrap_or(1.0);
    let floor = param(node, "floor").unwrap_or(3.6).max(2.4);
    layout
        .lots
        .iter()
        .map(|lot| {
            let height = (f64::from(lot.height) * scale).clamp(3.0, 300.0);
            if (18.0..32.0).contains(&height) {
                (((height - floor * 1.35) / floor).floor().max(0.0) as usize).saturating_mul(84)
            } else {
                0
            }
        })
        .sum()
}

/// The triangles a city node will draw, for the cost estimate (the layout is cheap, so it is really made).
pub(crate) fn estimate(node: &Node) -> usize {
    if node.op.family().as_str() == "street.road" {
        #[expect(clippy::cast_possible_truncation, reason = "bounded road length")]
        let segments = (param(node, "length").unwrap_or(80.0) / 5.0) as usize;
        return match text(node, "part") {
            "markings" | "crosswalk" => segments
                .saturating_mul(
                    usize::from(param(node, "lanes").unwrap_or(2.0) as u8).saturating_add(2),
                )
                .saturating_mul(2),
            "pavement" | "kerb" => 48,
            _ => 2,
        };
    }
    if node.op.family().as_str() == "street.prop" {
        return match (text(node, "kind"), text(node, "part")) {
            ("utility_mast", "accent") => 60,
            ("traffic_signal", "lens") => 36,
            ("bus_shelter", "accent") => 24,
            (_, "lens") => 12,
            (_, "accent") => 36,
            _ => 72,
        };
    }
    if node.op.family().as_str() == "building.asset" {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "floor and bay parameters are bounded by the operator spec"
        )]
        let floors = param(node, "floors").unwrap_or(8.0).max(2.0) as usize;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "floor and bay parameters are bounded by the operator spec"
        )]
        let bays = param(node, "bays").unwrap_or(6.0).max(2.0) as usize;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "detail is bounded by the operator spec"
        )]
        let detail = param(node, "detail").unwrap_or(2.0).max(0.0) as usize;
        let windows = floors.saturating_mul(bays).saturating_mul(4);
        return match text(node, "part") {
            "glass" => windows.saturating_mul(2),
            "doors" => 32,
            "trim" => {
                let style = text(node, "style");
                let per_window = 48
                    + usize::from(detail >= 1) * 12
                    + usize::from(detail >= 2) * 24
                    + usize::from(detail >= 3 && style != "glass") * 12;
                let band_and_crown = if style == "art_deco" {
                    12 + (bays + 1) * 12
                } else {
                    12
                };
                floors.saturating_mul(4).saturating_mul(
                    bays.saturating_mul(per_window)
                        .saturating_add(band_and_crown),
                )
            }
            "roof" => floors.saturating_mul(4).saturating_add(256),
            _ => windows.saturating_mul(8),
        };
    }
    let Ok((_, layout)) = lay_out(node) else {
        return 0;
    };
    let lots = layout.lots.len();
    // Only city.buildings has `detail`; city.ground has none.
    let detail = param(node, "detail").unwrap_or(0.0);
    // wings and mansard roofs: up to three buildings on a lot, and a slope with dormers on the roof
    let wings = 1.0 + 1.5 * param(node, "courtyards").unwrap_or(0.0);
    let mansard = param(node, "mansard").unwrap_or(0.0);
    let lots = (lots as f64 * wings) as usize;
    match (node.op.family().as_str(), text(node, "part")) {
        (_, "roof") => lots * (70 + (detail * 120.0 + mansard * 140.0) as usize),
        (_, "walls") => lots * (14 + (detail * 12.0 + mansard * 40.0) as usize),
        ("city.ground", "pavement") => lots * 2 + 400,
        ("city.ground", _) => 4,
        (_, "metal") => fire_escape_triangles(node, &layout, detail),
        (_, "awning") => lots * if detail >= 0.2 { 90 } else { 0 },
        (_, "shop") => lots * (8 + (detail * 60.0) as usize),
        _ => lots * 14,
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "bounded dimensions and detail fit f32/u8"
)]
#[expect(
    clippy::cast_sign_loss,
    reason = "operator validation bounds these whole-number parameters"
)]
fn building_asset(node: &Node) -> Result<Value, Fail> {
    let params = HeroAssetParams {
        at: triple(node, "at")?,
        width: param(node, "width")? as f32,
        depth: param(node, "depth")? as f32,
        floors: param(node, "floors")? as u8,
        bays: param(node, "bays")? as u8,
        floor_height: param(node, "floor_height")? as f32,
        detail: param(node, "detail")? as u8,
        style: text(node, "style").to_owned(),
        seed: param(node, "seed")?.max(0.0) as u64,
    };
    let built = build_hero_asset(&params).map_err(|error| {
        fail(
            "building.asset",
            format!("{}: {error}", node.op.family().as_str()),
        )
    })?;
    let part = match text(node, "part") {
        "glass" => &built.glass,
        "trim" => &built.trim,
        "roof" => &built.roof,
        "doors" => &built.doors,
        _ => &built.masonry,
    };
    if part.indices.is_empty() {
        return Err(fail(
            "building.empty_part",
            format!(
                "building.asset part '{}' has no geometry for this style",
                text(node, "part")
            ),
        ));
    }
    Ok(finish(part))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "lane count is bounded by the operator spec"
)]
fn street_road(node: &Node) -> Result<Value, Fail> {
    let params = ve_world::streetscape::RoadParams {
        at: triple(node, "at")?,
        length: param(node, "length")? as f32,
        width: param(node, "width")? as f32,
        lanes: param(node, "lanes")? as u8,
        axis: text(node, "axis").to_owned(),
        part: text(node, "part").to_owned(),
    };
    let mesh = ve_world::streetscape::build_road(&params)
        .map_err(|error| fail("street.road", error.to_string()))?;
    Ok(finish(&mesh))
}

fn street_prop(node: &Node) -> Result<Value, Fail> {
    let params = ve_world::streetscape::PropParams {
        at: triple(node, "at")?,
        #[expect(clippy::cast_possible_truncation, reason = "bounded prop height")]
        height: param(node, "height")? as f32,
        axis: text(node, "axis").to_owned(),
        kind: text(node, "kind").to_owned(),
        part: text(node, "part").to_owned(),
    };
    let mesh = ve_world::streetscape::build_prop(&params)
        .map_err(|error| fail("street.prop", error.to_string()))?;
    Ok(finish(&mesh))
}

fn convex_hull(points: &mut Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut hull: Vec<[f32; 2]> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        for p in points.iter() {
            while hull.len() >= start + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], *p) <= 0.0
            {
                hull.pop();
            }
            hull.push(*p);
        }
        hull.pop();
        if pass == 0 {
            points.reverse();
        }
    }
    hull
}

fn finish(strips: &Strips) -> Value {
    let mut value = Value::solid(
        Mesh {
            positions: strips.positions.clone(),
            indices: strips.indices.clone(),
        },
        0.0,
    );
    value.uvs = strips.uvs.clone();
    value
}

fn ground(node: &Node) -> Result<Value, Fail> {
    let (params, layout) = lay_out(node)?;
    let mut strips = Strips::default();
    if text(node, "part") == "pavement" {
        let mut by_block = std::collections::BTreeMap::<u32, Vec<[f32; 2]>>::new();
        for lot in &layout.lots {
            by_block
                .entry(lot.block)
                .or_default()
                .extend(lot.footprint.iter().copied());
        }
        let pavement = (params.street_width * 0.3).min(2.2);
        for mut points in by_block.into_values() {
            let hull = convex_hull(&mut points);
            if hull.len() >= 3 {
                let outline = inset(&hull, -pavement);
                strips.walls(&outline, 0.0, 0.16, 3.6);
                strips.cap(&outline, 0.16);
            }
        }
    } else {
        let (hx, hz) = (params.size[0] * 0.5 + 6.0, params.size[1] * 0.5 + 6.0);
        strips.cap(&[[-hx, -hz], [hx, -hz], [hx, hz], [-hx, hz]], 0.0);
    }
    Ok(finish(&strips))
}

#[expect(clippy::cast_possible_truncation, reason = "ratios and metres fit f32")]
#[expect(clippy::cast_sign_loss, reason = "styles are small whole numbers")]
fn buildings(node: &Node) -> Result<Value, Fail> {
    let (params, layout) = lay_out(node)?;
    let part = text(node, "part");
    let styles = param(node, "styles")?.max(1.0) as u64;
    let style = param(node, "style")?.max(0.0) as u64;
    if style >= styles {
        return Err(fail(
            "param.range",
            format!("city.buildings style {style} must be below styles {styles}"),
        ));
    }
    let shape = Shape {
        seed: params.seed,
        floor: param(node, "floor")? as f32,
        clutter: param(node, "clutter")? as f32,
        detail: param(node, "detail")? as f32,
        scale: param(node, "height_scale")? as f32,
        height_min: param(node, "height_min")? as f32,
        height_max: param(node, "height_max")? as f32,
        mansard: param(node, "mansard")? as f32,
        courtyards: param(node, "courtyards")? as f32,
    };
    if shape.height_min > shape.height_max {
        return Err(fail(
            "param.range",
            format!(
                "city.buildings height_min {} must not be above height_max {}",
                shape.height_min, shape.height_max
            ),
        ));
    }
    let mut strips = Strips::default();
    for (index, lot) in layout.lots.iter().enumerate() {
        if style_of(index, params.seed, styles) != style {
            continue;
        }
        for building in lot_buildings(lot, index, &shape) {
            // a lot that cannot be a building (a sliver, a concave one) is left as a gap rather than failing the district
            let Ok(built) = build(&building) else {
                continue;
            };
            let source = match part {
                "shop" => &built.shop,
                "roof" => &built.roof,
                "metal" => &built.metal,
                "awning" => &built.awning,
                _ => &built.walls,
            };
            let base = u32::try_from(strips.positions.len()).unwrap_or(0);
            strips.positions.extend_from_slice(&source.positions);
            strips
                .indices
                .extend(source.indices.iter().map(|i| i + base));
            strips.uvs.extend_from_slice(&source.uvs);
        }
    }
    if strips.indices.is_empty() {
        if matches!(part, "metal" | "awning") {
            return Err(fail(
                "city.empty",
                "this part is empty: fire escapes (part metal) need detail 0.34 or more and buildings between 18 and 32 m, awnings (part awning) need detail 0.2 or more",
            ));
        }
        return Err(fail(
            "city.empty",
            "no building of this style: lower styles, or choose another style",
        ));
    }
    Ok(finish(&strips))
}
