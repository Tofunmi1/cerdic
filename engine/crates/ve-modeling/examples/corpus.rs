//! The production evaluator runs the same primitive/boolean graph natively and in WASI.
#![expect(clippy::print_stdout, reason = "parity corpus output")]

use ve_geometry::{AttrData, Attribute, Domain, PolyMesh, SubdivisionOptions, subdivide};
use ve_graph::{Outcome, Session};
use ve_kernel::{Crease, ExecutionContext};
use ve_modeling::{Evaluator, Quality};

fn text_case(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let parsed = ve_graph::parse_text(&ve_graph::builtin_registry(), source)
        .map_err(|error| format!("{error:?}"))?;
    let mut session = Session::with_builtin(1);
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":0,"transaction":"t_instances","commands":parsed.commands});
    if !matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ) {
        return Err("instance parity fixture rejected".into());
    }
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| error.message)?;
    for part in result.parts {
        println!(
            "{}",
            serde_json::json!({"node":part.node,"volume":part.volume,"uvs":part.uvs,
            "positions":part.positions,"indices":part.indices,"occlusion":part.occlusion,"transforms":part.transforms,"instance_indices":part.instance_indices,
            "animation":part.animation,"behavior":part.behavior,"parent":part.parent,"bone":part.bone,"ik":part.ik,"body":part.body,
            "skin":part.skin.as_ref().map(|s| serde_json::json!({"bones":s.bones,"falloff":s.falloff,"influences":s.influences,"blend":s.blend,"paint":s.paint,"joints":s.joints,"weights":s.weights})),"warnings":&result.warnings})
        );
    }
    Ok(())
}

fn opacity_case() -> Result<(), Box<dyn std::error::Error>> {
    let source = "pane = solid.box@1(size=[1,0.08,1])\nglass = solid.material@1(pane, color=[0.1,0.3,0.9], opacity=0.35)\noutput glass";
    let parsed = ve_graph::parse_text(&ve_graph::builtin_registry(), source)
        .map_err(|error| format!("{error:?}"))?;
    let mut session = Session::with_builtin(1);
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":0,"transaction":"t_opacity","commands":parsed.commands});
    if !matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ) {
        return Err("opacity parity fixture rejected".into());
    }
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| error.message)?;
    println!(
        "{}",
        serde_json::json!({
            "case":"g10_material_opacity",
            "materials":result.parts.iter().map(|part| &part.material).collect::<Vec<_>>()
        })
    );
    Ok(())
}

fn attribute_warning_case() {
    let warning = ve_modeling::EvaluationWarning::attribute_dropped("n_0123abcd", "tag");
    println!("{}", serde_json::json!({"warnings":[warning]}));
}

fn subdivision_channel_case() -> Result<(), Box<dyn std::error::Error>> {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    let mut cage = PolyMesh::new(
        positions.clone(),
        vec![4; 6],
        vec![
            0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 3, 7, 6, 2, 0, 4, 7, 3, 1, 2, 6, 5,
        ],
    )?;
    for (name, coordinate) in [("weight_x", 0), ("weight_y", 1)] {
        cage.set_attribute(
            name,
            Attribute {
                domain: Domain::Point,
                data: AttrData::F32(positions.iter().map(|point| point[coordinate]).collect()),
            },
        )?;
    }
    add_uv_islands(&mut cage)?;
    let context = ExecutionContext::new()?;
    let refined = subdivide(
        &cage,
        SubdivisionOptions {
            levels: 1,
            boundary_rule: 0,
            crease_method: 0,
            creases: vec![Crease {
                vertices: [0, 1],
                sharpness: 2.0,
            }],
            sharp_corners: Vec::new(),
        },
        &context,
    )?;
    emit_subdivision_channels(&refined);
    Ok(())
}

fn add_uv_islands(cage: &mut PolyMesh) -> Result<(), Box<dyn std::error::Error>> {
    let mut u = Vec::new();
    let mut v = Vec::new();
    for face in 0..6 {
        let offset = [0.0, 2.0, 4.0, 6.0, 8.0, 10.0][face];
        for [corner_u, corner_v] in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            u.push(corner_u + offset);
            v.push(corner_v);
        }
    }
    for (name, values) in [("u", u), ("v", v)] {
        cage.set_attribute(
            name,
            Attribute {
                domain: Domain::Corner,
                data: AttrData::F32(values),
            },
        )?;
    }
    Ok(())
}

fn emit_subdivision_channels(refined: &ve_geometry::Subdivided) {
    let AttrData::F32(point_x) = &refined.mesh.attributes()["weight_x"].data else {
        unreachable!()
    };
    let AttrData::F32(point_y) = &refined.mesh.attributes()["weight_y"].data else {
        unreachable!()
    };
    let AttrData::F32(uv_u) = &refined.mesh.attributes()["u"].data else {
        unreachable!()
    };
    let AttrData::F32(uv_v) = &refined.mesh.attributes()["v"].data else {
        unreachable!()
    };
    println!(
        "{}",
        serde_json::json!({
            "case":"g7_uv_and_point_channels",
            "positions":refined.mesh.positions(),
            "face_sizes":refined.mesh.face_sizes(),
            "corners":refined.mesh.corners(),
            "point_x":point_x,
            "point_y":point_y,
            "uv_u":uv_u,
            "uv_v":uv_v
        })
    );
}

fn surface_cases() -> Result<(), Box<dyn std::error::Error>> {
    text_case(
        "section = profile.circle(radius=0.1, segments=16)\npath = curve.circle(center=[2,0,0], radius=1, segments=16)\nring = solid.sweep(path, section)\noutput ring",
    )?;
    text_case(
        "a = profile.circle(radius=0.2, segments=12)\nb = profile.circle(radius=0.1, segments=12)\nbody = solid.loft(a, b, spacing=0.3)\noutput body",
    )?;
    text_case(
        "profile = profile.circle(radius=0.05, segments=12)\npath = curve.bezier(points=[[0,0,0],[0,0.3,0],[0.3,0.7,0],[0.3,1,0]], steps=8)\npipe = solid.sweep(path, profile)\noutput pipe",
    )?;
    // Physics bodies: a dynamic ball, a static mesh floor, and a body dropped because the part is animated.
    text_case(
        "ball = solid.sphere(radius=0.3, segments=12)\nb = phys.body(ball, restitution=0.7, density=2, linear_damping=0.1)\noutput b\nfloor = solid.box(size=[8,0.2,8])\nf = phys.body(floor, kind=\"static\", shape=\"mesh\", friction=1.2)\noutput f\ncrate = solid.box(size=[0.5,0.5,0.5])\nslide = anim.track(crate, property=\"position\", keys=\"0:0,0,0; 1:1,0,0\")\nd = phys.body(slide, shape=\"box\")\noutput d",
    )?;
    // Joints: a hinged door with limits and a motor, a spring-hung weight, a collide trigger, and a joint to nowhere.
    text_case(
        "post = solid.box(size=[0.2,2,0.2])\npost_b = phys.body(post, kind=\"static\")\noutput post = post_b\ndoor = solid.box(size=[1,1.8,0.05])\ndoor_at = xform.move(door, by=[0.6,0,0])\ndoor_b = phys.body(door_at, shape=\"box\")\nhinge = phys.joint(door_b, kind=\"hinge\", to=\"post\", anchor=[0.1,0,0], axis=[0,1,0], limited=true, low=-90, high=100, motor_speed=20, motor_force=5)\nspring = phys.joint(hinge, kind=\"spring\", anchor=[0.6,0.9,0], anchor_b=[0.6,2.5,0], rest_length=1.2, stiffness=15, damping=2)\nlost = phys.joint(spring, to=\"ghost\", anchor=[0,0,0])\noutput door = lost\nball = solid.sphere(radius=0.2, segments=10)\nball_b = phys.body(ball)\nrings = behavior.on(ball_b, trigger=\"collide\", accepts=\"post\", sets=\"ring\")\noutput ball = rings",
    )?;
    // Terrain: hills, ridges with an island edge and lake beds, and copies dropped onto the hills.
    text_case(
        "land = terrain.heightfield(size=[4,4], height=1.2, scale=2, octaves=4, roughness=0.55, seed=9, resolution=12, base=0.3)\noutput land",
    )?;
    text_case(
        "land = terrain.heightfield(size=[6,5], height=2, scale=3, style=\"ridged\", edge_falloff=0.5, flat_below=0.2, seed=2, resolution=16)\noutput land",
    )?;
    text_case(
        "land = terrain.heightfield(size=[4,4], height=1.2, scale=2, seed=9, resolution=12)\ntree = solid.cone(radius=0.1, height=0.4, segments=6)\ncrowd = inst.scatter(tree, count=12, width=5, depth=5, min_spacing=0.2, seed=3)\nplaced = inst.drop(crowd, land, offset=-0.02, align=0.7)\noutput placed",
    )?;
    text_case(
        "lake = water.surface(size=[8,6], level=0.8, depth=1.2, wave_height=0.04, wave_length=2, seed=4, resolution=12)\noutput lake",
    )?;
    text_case(
        "sky = scene.sky(sun_elevation=18, sun_azimuth=-35, turbidity=6, clouds=true, cloud_cover=0.55, seed=9)\noutput sky",
    )?;
    // Curve tools: each swept with a profile so the sampled points show up in the mesh.
    for curve in [
        "path = curve.spline(points=[[0,0,0],[0.4,0.5,0.1],[0.9,0.6,-0.2],[1.2,1.2,0.3]], steps=6)",
        "path = curve.helix(radius=0.4, end_radius=0.2, pitch=0.25, turns=2, segments_per_turn=20)",
        "route = curve.polyline(points=[[0,0,0],[1,0,0],[1,1,0],[0,1,0.5]])\npath = curve.fillet(route, distance=0.25, steps=5)",
        "route = curve.spline(points=[[0,0,0],[0.5,0.8,0],[1,0,0.4]], steps=10)\nfine = curve.resample(route, count=13)\npath = curve.reverse(fine)",
    ] {
        text_case(&format!(
            "{curve}\nring = profile.circle(radius=0.03, segments=10)\ntube = solid.sweep(path, ring, scale_end=0.4, twist=0.8)\noutput tube"
        ))?;
    }
    text_case(
        "box = solid.box(size=[0.2,1,0.2])\ntwist = xform.twist(box, angle=0.7)\noutput twist",
    )?;
    text_case(
        "box = solid.box(size=[0.2,1,0.2])\ntaper = xform.taper(box, top_scale=0.6)\noutput taper",
    )?;
    text_case(
        "box = solid.box(size=[0.2,1,0.2])\nlean = xform.shear(box, x_shift=0.3, z_shift=-0.1)\noutput lean",
    )?;
    text_case(
        "panel = solid.box(size=[1,1,0.05])\nflag = xform.wave(panel, amplitude=0.1, wavelength=0.5, phase=0.6, direction=\"z\", tolerance=0.005)\noutput flag",
    )?;
    text_case(
        "blob = solid.box(size=[1,1,1])\nrock = xform.noise(blob, amplitude=0.03, scale=0.6, seed=5, tolerance=0.02)\noutput rock",
    )?;
    text_case(
        "box = solid.box(size=[0.2,1,0.2])\nsquash = xform.lattice(box, divisions=3, offsets=\"1 2 1 0 -0.2 0; 1 0 1 0 0.1 0; 0 1 0 -0.05 0 0.03\", tolerance=0.01)\noutput squash",
    )?;
    text_case("box = solid.box(size=[0.2,1,0.2])\nbend = xform.bend(box, angle=0.4)\noutput bend")?;
    text_case(
        "section = profile.circle(radius=0.03, segments=12)\npath = curve.polyline(points=[[1,0,0],[0,0,1],[-1,0,0],[0,0,-1]], closed=true)\nring = solid.sweep(path, section)\noutput ring",
    )?;
    text_case(
        "section = profile.circle(radius=0.02, segments=12)\npath = curve.bezier(points=[[0.5,0,0],[0.5,0,0.5],[-0.5,0,0.5],[0.5,0,0]], steps=24, closed=true)\nloop = solid.sweep(path, section)\noutput loop",
    )?;
    text_case("box = solid.box(size=[1,1,1])\ncut = solid.chamfer(box, width=0.12)\noutput cut")?;
    colour_cases()?;
    polygon_cases()
}

/// Current field vocabulary: primitive, torus, ellipsoid and a smooth composition.
fn field_cases() -> Result<(), Box<dyn std::error::Error>> {
    text_case(
        "field = field.sphere@1(radius=0.5)\nsolid = field.to_solid@1(field, min=[-1,-1,-1], max=[1,1,1], edge_length=0.1)\noutput solid",
    )?;
    text_case(
        "field = field.torus@1(major=0.5, tube=0.15)\nsolid = field.to_solid@1(field, min=[-1,-1,-1], max=[1,1,1], edge_length=0.12)\noutput solid",
    )?;
    text_case(
        "field = field.ellipsoid@1(radii=[0.6,0.4,0.3])\nsolid = field.to_solid@1(field, min=[-1,-1,-1], max=[1,1,1], edge_length=0.12)\noutput solid",
    )?;
    text_case(
        "a = field.sphere@1(radius=0.45)\nb = field.sphere@1(radius=0.35)\nright = field.move@1(b, by=[0.4,0,0])\nblend = field.smooth_union@1(a, right, radius=0.2)\nsolid = field.to_solid@1(blend, min=[-1,-1,-1], max=[1,1,1], edge_length=0.12)\noutput solid",
    )?;
    text_case(
        "a = field.capsule@1(radius=0.15, half_length=0.4)\nb = field.sphere@1(radius=0.3)\nhead = field.move@1(b, by=[0,0.55,0])\nbody = field.smooth_union@1(a, head, radius=0.12)\nsolid = field.to_solid@1(body, min=[-1,-1,-1], max=[1,1,1], edge_length=0.12)\noutput solid",
    )?;
    text_case(
        "field = field.tube@1(points=[[-0.5,0,0],[0,0.3,0],[0.5,0,0]], radius=0.12)\nsolid = field.to_solid@1(field, min=[-1,-1,-1], max=[1,1,1], edge_length=0.16)\noutput solid",
    )?;
    text_case(
        "sphere = field.sphere@1(radius=0.2)\nfield = field.repeat@1(sphere, period=[0.8,1,1], count=[1,0,0])\nsolid = field.to_solid@1(field, min=[-1.3,-0.8,-0.8], max=[1.3,0.8,0.8], edge_length=0.16)\noutput solid",
    )
}

include!("corpus/part_1.rs");
include!("corpus/part_2.rs");
