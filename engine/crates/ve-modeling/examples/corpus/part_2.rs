/// Polygon editing: every operator family, identical natively and in WASI.
fn polygon_cases() -> Result<(), Box<dyn std::error::Error>> {
    text_case(
        "box = solid.box@1(size=[1,1,1])\ntris = poly.from_solid@1(box)\nm = poly.mirror@1(tris, point=[0,0,0], normal=[1,0,0])\nsolid = poly.to_solid@1(m)\noutput solid",
    )?;
    // G10-10: a grid revolved into a closed ring, included in native/WASI parity.
    text_case(
        "strip = poly.grid@1(width=0.2, height=1.0, columns=1, rows=4)\nspun = poly.spin@1(strip, center=[-1,0,0], axis=[0,1,0], angle=6.283185307179586, steps=64, merge=true)\nsolid = poly.to_solid@1(spun)\noutput solid",
    )?;
    text_case(
        "box = solid.box@1(size=[1,1,1])\nmesh = poly.from_solid@1(box)\ncut = poly.bisect@1(mesh, point=[0,0.2,0], normal=[0,1,0], keep=\"below\")\nsolid = poly.to_solid@1(cut)\noutput solid",
    )?;
    // G11-11: loop cut a closed ring of merged box quads.
    text_case(
        "box = solid.box@1(size=[1,1,1])\ntris = poly.from_solid@1(box)\nmesh = poly.merge_coplanar@1(tris)\ncut = poly.loop_cut@1(mesh, at=[0.5,0,0.5], cuts=1)\noutput cut",
    )?;
    // G10-09: UV packing an automatically unwrapped cube, included in native/WASI parity.
    text_case(
        "box = solid.box@1(size=[1,1,1])\nmesh = poly.from_solid@1(box)\nuv = poly.unwrap_uv@1(mesh, pack=false)\npacked = poly.pack_uv@1(uv, margin=0.02, rotate=false)\nsolid = poly.to_solid@1(packed)\noutput solid",
    )?;
    text_case(
        "box = solid.box(size=[1,1,1])\nmesh = poly.from_solid(box)\ntop = poly.select(mesh, by=\"normal\", direction=[0,1,0], angle=0.05)\nquad = poly.dissolve_faces(mesh, top)\nface = poly.select(quad, by=\"normal\", direction=[0,1,0], angle=0.05, expect=\"one\")\ninset = poly.inset_faces(quad, face, distance=0.1)\ncap = poly.select(inset, by=\"attribute\", attribute=\"role\", value=1)\nraised = poly.extrude_faces(inset, cap, offset=[0,0.2,0])\nout = poly.to_solid(raised)\noutput out",
    )?;
    text_case(
        "wall = poly.grid(width=3, height=2, columns=3, rows=2)\ncells = poly.select(wall)\nframed = poly.inset_faces(wall, cells, distance=0.1)\nwindows = poly.select(framed, by=\"attribute\", attribute=\"role\", value=1)\nfacade = poly.extrude_faces(framed, windows, offset=[0,0,0.1], individual=true)\noutput facade",
    )?;
    text_case(
        "box = solid.box(size=[1,1,1])\ntriangles = poly.from_solid(box)\nmesh = poly.merge_coplanar(triangles)\nedge = poly.select(mesh, element=\"edge\", by=\"box\", box_min=[-0.1,0.4,0.4], box_max=[0.1,0.6,0.6], expect=\"one\")\nround = poly.bevel_edges(mesh, edge, width=0.2, segments=3)\nout = poly.to_solid(round)\noutput out",
    )?;
    text_case(
        "box = solid.box(size=[1,1,1])\nother = xform.move(box, by=[3,0,0])\nboth = solid.union(box, other)\ntriangles = poly.from_solid(both)\nmesh = poly.merge_coplanar(triangles)\na = poly.select(mesh, by=\"box\", box_min=[0.4,-1,-1], box_max=[0.6,1,1], expect=\"one\")\nopen_a = poly.delete_faces(mesh, a)\nb = poly.select(open_a, by=\"box\", box_min=[2.4,-1,-1], box_max=[2.6,1,1], expect=\"one\")\nopen_b = poly.delete_faces(open_a, b)\nends = poly.select(open_b, element=\"loop\")\nbridged = poly.bridge_loops(open_b, ends)\nout = poly.to_solid(bridged)\noutput out",
    )?;
    text_case(
        "box = solid.box(size=[1,1,1])\ntriangles = poly.from_solid(box)\ncage = poly.merge_coplanar(triangles)\nsmooth = poly.subdivide(cage, levels=2)\noutput smooth",
    )?;
    text_case(
        "profile = profile.polygon(points=[[0,0],[2,0],[2,2],[1,0.8],[0,2],[-0.5,1],[-0.5,0.25]])\nsolid = solid.extrude(profile, height=1)\ntriangles = poly.from_solid(solid)\ncage = poly.merge_coplanar(triangles)\nsmooth = poly.subdivide(cage, levels=1)\noutput smooth",
    )?;
    text_case(
        "grid = poly.grid(width=2, height=2, columns=4, rows=4)\nsmooth = poly.subdivide(grid, levels=2, boundary=\"edges\")\noutput smooth",
    )?;
    // Keyframed tracks: a hinged door and a pulsing, spinning cube (tracks are data, so output must match exactly).
    text_case(
        "door = solid.box(size=[0.9,2,0.05])\nswing = anim.track(door, property=\"rotation\", keys=\"0:0,0,0; 1.5:0,90,0\", pivot=[-0.45,0,0], loop=false)\noutput swing",
    )?;
    text_case(
        "cube = solid.box(size=[0.5,0.5,0.5])\npulse = anim.track(cube, property=\"scale\", keys=\"0:1,1,1; 0.5:1.5,1.5,1.5; 1:1,1,1\")\nspin = anim.track(pulse, property=\"rotation\", keys=\"0:0,0,0; 4:0,360,0\", interpolation=\"linear\")\noutput spin",
    )?;
    // A behaviour rides with its part: a door that opens when the viewer comes near.
    text_case(
        "door = solid.box(size=[0.9,2,0.05])\nswing = anim.track(door, property=\"rotation\", keys=\"0:0,0,0; 1:0,90,0\", pivot=[-0.45,0,0], loop=false)\nreact = behavior.on(swing, trigger=\"near\", action=\"toggle\", radius=3)\noutput react",
    )?;
    // A switch that names the part it runs, and the listening door.
    text_case(
        "door = solid.box(size=[0.9,2,0.05])\nswing = anim.track(door, property=\"rotation\", keys=\"0:0,0,0; 1:0,90,0\", pivot=[-0.45,0,0], loop=false)\nlisten = behavior.on(swing, trigger=\"signal\")\noutput front_door = listen\nbutton = solid.box(size=[0.1,0.1,0.02])\npress = behavior.on(button, trigger=\"select\", target=\"front_door\")\noutput door_button = press",
    )?;
    // An arm: a child follows its parent and bends at its own elbow; a missing parent is dropped with a warning.
    text_case(
        "upper = solid.box(size=[0.1,0.6,0.1])\nshoulder = anim.track(upper, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,45\", pivot=[0,0.3,0])\noutput upper_arm = shoulder\nlower = solid.box(size=[0.1,0.5,0.1])\nlow = xform.move(lower, by=[0,-0.55,0])\nattached = anim.parent(low, parent=\"upper_arm\")\nelbow = anim.track(attached, property=\"rotation\", keys=\"0:0,0,0; 1:90,0,0\", pivot=[0,-0.3,0])\noutput forearm = elbow\nstray = solid.box(size=[0.1,0.1,0.1])\nlost = anim.parent(stray, parent=\"ghost\")\noutput stray_out = lost",
    )?;
    // A lamp: glow and visibility tracks, an eased track and a wave (all plain data).
    text_case(
        "bulb = solid.sphere(radius=0.1)\nlit = solid.material(bulb, color=[1,0.9,0.6], emissive=[2,1.8,1.2], emits_light=true)\nfade = anim.track(lit, property=\"glow\", keys=\"0:0; 0.3:1\", loop=false)\nflick = anim.wave(fade, property=\"glow\", amplitude=[0.1,0,0], period=0.5, shape=\"triangle\")\noutput lamp = flick\ncube = solid.box(size=[0.2,0.2,0.2])\nbob = anim.wave(cube, property=\"position\", amplitude=[0,0.15,0], period=2)\nvanish = anim.track(bob, property=\"visible\", keys=\"0:1; 1:0\", interpolation=\"step\", loop=false)\neased = anim.track(vanish, property=\"scale\", keys=\"0:1,1,1; 1:2,2,2\", interpolation=\"ease_in\")\noutput cube_out = eased",
    )?;
    // Flag logic: a locked door, its key, a lamp that follows a switch, and a clipless gate.
    text_case(
        "door = solid.box(size=[0.9,2,0.05])\nswing = anim.track(door, property=\"rotation\", keys=\"0:0,0,0; 1:0,90,0\", pivot=[-0.45,0,0], loop=false)\nlatch = behavior.on(swing, trigger=\"select\", needs=\"has_key; !alarm\")\noutput front_door = latch\nkey = solid.box(size=[0.1,0.05,0.02])\nvanish = anim.track(key, property=\"visible\", keys=\"0:1; 0.2:0\", interpolation=\"step\", loop=false)\ntake = behavior.on(vanish, trigger=\"select\", action=\"play\", sets=\"has_key\")\noutput brass_key = take\nbulb = solid.sphere(radius=0.1)\nfade = anim.track(bulb, property=\"glow\", keys=\"0:0; 0.3:1\", loop=false)\nmirror = behavior.on(fade, trigger=\"flag\", watch=\"powered\", action=\"follow\")\noutput lamp = mirror\nbox = solid.box(size=[0.1,0.1,0.05])\nflip = behavior.on(box, sets=\"powered=toggle\")\noutput switch = flip\nmarker = solid.box(size=[0.1,0.1,0.1])\ngate = behavior.on(marker, trigger=\"flag\", watch=\"a\", needs=\"b\", sets=\"c=toggle\")\noutput gate_out = gate",
    )?;
    // Grab and hold triggers, and an animated colour.
    text_case(
        "key = solid.box(size=[0.12,0.05,0.02])\ntake = behavior.on(key, trigger=\"grab\", sets=\"has_key\")\noutput brass_key = take\ncube = solid.box(size=[0.3,0.3,0.3])\ngold = solid.material(cube, color=[0.9,0.7,0.05])\nfade = anim.track(gold, property=\"color\", keys=\"0:0.9,0.7,0.05; 1:0.1,0.1,0.9\", interpolation=\"linear\", loop=false)\npick = behavior.on(fade, trigger=\"hold\", sets=\"holding\")\noutput gold_cube = pick",
    )?;
    // Curve interpolation: Blender-style automatic Bezier handles and easing, as plain track data.
    text_case(
        "a = solid.box(size=[1,1,1])\ncurve = anim.track(a, property=\"position\", keys=\"0:0,0,0; 1:1,2,0; 3:4,0,1; 4:2,3,0\", interpolation=\"bezier\", loop=false)\noutput curved = curve\nb = solid.box(size=[1,1,1])\nspring = anim.track(b, property=\"scale\", keys=\"0:1,1,1; 1:2,2,2\", interpolation=\"elastic\", loop=false)\nbounce = anim.track(spring, property=\"position\", keys=\"0:0,0,0; 1:0,1,0\", interpolation=\"bounce\", loop=false)\nback = anim.track(bounce, property=\"rotation\", keys=\"0:0,0,0; 1:0,90,0\", interpolation=\"back\", loop=false)\noutput eased = back",
    )?;
    // Clips and sequences: two named clips on an arm chosen by senders, and a part that chains three clips.
    text_case(
        "arm = solid.box(size=[0.1,0.6,0.1])\nwave = anim.track(arm, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,40\", clip=\"wave\", loop=false)\nreach = anim.track(wave, property=\"rotation\", keys=\"0:0,0,0; 2:60,0,0\", clip=\"reach\", loop=false)\nlisten = behavior.on(reach, trigger=\"signal\")\noutput arm_out = listen\nb = solid.box(size=[0.1,0.1,0.1])\nbutton = behavior.on(b, trigger=\"select\", target=\"arm_out:wave\")\noutput button_out = button\ncube = solid.box(size=[0.2,0.2,0.2])\na1 = anim.track(cube, property=\"position\", keys=\"0:0,0,0; 0.5:1,0,0\", clip=\"a\", loop=false)\nb1 = anim.track(a1, property=\"position\", keys=\"0:0,0,0; 0.5:0,1,0\", clip=\"b\", loop=false)\nc1 = anim.track(b1, property=\"position\", keys=\"0:0,0,0; 0.5:0,0,1\", clip=\"c\", loop=false)\ngo = behavior.on(c1, trigger=\"select\", clip=\"a\", then=\"b; c\", action=\"play\")\noutput runner = go",
    )?;
    // An IK chain: a working one (its segments resolved), and one dropped because its bones are not joined.
    text_case(
        "u = solid.box(size=[0.1,0.1,0.1])\nupper_bone = rig.bone(u, head=[0,0,0], tail=[1,0,0])\noutput upper_arm = upper_bone\nf = solid.box(size=[0.1,0.1,0.1])\nfore_bone = rig.bone(f, head=[1,0,0], tail=[2,0,0])\nfore_child = anim.parent(fore_bone, parent=\"upper_arm\")\noutput forearm = fore_child\nball = solid.sphere(radius=0.1)\ngoal = rig.ik(ball, bones=\"upper_arm; forearm\", iterations=48)\noutput target = goal\nb = solid.sphere(radius=0.1)\nbroken = rig.ik(b, bones=\"upper_arm; ghost\")\noutput lost_target = broken",
    )?;
    // A placed set with a phased sway: copies share one track, each plays it a little ahead or behind.
    text_case(
        "reed = solid.box(size=[0.05,0.8,0.05])\nstanding = xform.move(reed, by=[0,0.4,0])\nrow = inst.array(standing, count_x=6, spacing=[0.2,0,0])\nsway = anim.wave(row, property=\"rotation\", amplitude=[0,0,12], period=3, phase_step=0.25, phase_jitter=0.5)\noutput reeds = sway",
    )?;
    // A socket and the key it accepts.
    text_case(
        "k = solid.box(size=[0.2,0.2,0.2])\nkey = behavior.on(k, trigger=\"hold\", sets=\"carried\")\noutput key_out = key\nl = solid.box(size=[0.3,0.3,0.05])\nlock = behavior.on(l, trigger=\"socket\", accepts=\"key_out\", radius=0.4, sets=\"unlocked\")\noutput lock_out = lock",
    )?;
    // Timers: one-shot and repeating (plain data; the firing steps are checked by a unit test).
    text_case(
        "door = solid.box(size=[0.9,2,0.05])\nswing = anim.track(door, property=\"rotation\", keys=\"0:0,0,0; 0.5:0,90,0\", pivot=[-0.45,0,0], loop=false)\nauto = behavior.on(swing, trigger=\"timer\", action=\"play\", after=2)\noutput auto_door = auto\ntick = solid.box(size=[0.1,0.1,0.1])\nblink = behavior.on(tick, trigger=\"timer\", every=0.5, sets=\"lit=toggle\")\noutput blinker = blink",
    )?;
    // Skinning: a tube cut into rings, weighted to two bones (weights must be bit-identical on every target), a
    // skin naming a missing bone, and a skin that is dropped because it also moves on its own.
    text_case(
        "lower = solid.box(size=[0.05,0.05,0.05])\nlower_pos = xform.move(lower, by=[0,-0.25,0])\nlower_rig = rig.bone(lower_pos, head=[0,-0.5,0], tail=[0,0,0])\nswing = anim.track(lower_rig, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,90\", interpolation=\"bezier\", pivot=[0,0,0], loop=false)\noutput lower_bone = swing\nupper = solid.box(size=[0.05,0.05,0.05])\nupper_pos = xform.move(upper, by=[0,0.25,0])\nupper_rig = rig.bone(upper_pos, head=[0,0,0], tail=[0,0.5,0])\noutput upper_bone = upper_rig\ntall = solid.box(size=[0.2,1,0.2])\ncage = poly.from_solid(tall)\nflat = poly.merge_coplanar(cage)\nuprights = poly.select(flat, element=\"edge\", by=\"box\", box_min=[-1,-0.1,-1], box_max=[1,0.1,1])\ncut = poly.subdivide_edges(flat, uprights, cuts=7)\nskinned = rig.skin(cut, bones=\"lower_bone; upper_bone; ghost\", falloff=2, influences=3, blend=\"dual_quaternion\", paint=\"upper_bone 0 0.4 0 0.3 1; lower_bone 0 -0.4 0 0.2 -0.5\")\noutput sleeve = skinned\nmoving = solid.box(size=[0.2,0.2,0.2])\nbad = rig.skin(moving, bones=\"lower_bone\")\nslide = anim.track(bad, property=\"position\", keys=\"0:0,0,0; 1:1,0,0\")\noutput not_skinned = slide",
    )?;
    // Dual-quaternion deformation of a few vertices (must be bit-identical on every target).
    let turn = |degrees: f32| {
        let (s, c) = degrees.to_radians().sin_cos();
        [
            c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.3, -0.2, 0.1, 1.0,
        ]
    };
    let poses = [turn(0.0), turn(70.0), turn(-130.0), turn(170.0)];
    let prepared = ve_modeling::rig::prepare_dual(&poses);
    for (point, joints, weights) in [
        (
            [0.1_f32, 0.7, 0.2],
            [0_u16, 1, 0, 0],
            [0.5_f32, 0.5, 0.0, 0.0],
        ),
        ([-0.3, 1.1, 0.0], [1, 2, 3, 0], [0.4, 0.3, 0.3, 0.0]),
        ([0.0, 0.2, 0.9], [3, 2, 1, 0], [0.25, 0.25, 0.25, 0.25]),
    ] {
        let (p, n) =
            ve_modeling::rig::deform_prepared(point, [0.0, 1.0, 0.0], joints, weights, &prepared);
        println!("{}", serde_json::json!({"dual_deform": {"p": p, "n": n}}));
    }
    // Automatic seams: a sphere (closed, cut along a path) and a cylinder (sharp rims) with no seams selected.
    text_case(
        "ball = solid.sphere(radius=1, segments=24)\ntriangles = poly.from_solid(ball)\nuv = poly.unwrap_uv(triangles)\nsolid = poly.to_solid(uv)\noutput solid",
    )?;
    text_case(
        "can = solid.cylinder(height=1, radius=0.4, segments=20)\ntriangles = poly.from_solid(can)\nuv = poly.unwrap_uv(triangles, method=\"angle\")\nsolid = poly.to_solid(uv)\noutput solid",
    )?;
    text_case(
        "box = solid.box(size=[1,1,1])\npaint = solid.material(box, color=[0.8,0.8,0.8])\ntriangles = poly.from_solid(paint)\ncage = poly.merge_coplanar(triangles)\nseams = poly.select(cage, element=\"edge\", by=\"sharp\", angle=0.5)\nuv = poly.unwrap_uv(cage, seams, method=\"angle\", rotate=true)\nsolid = poly.to_solid(uv)\ntextured = solid.texture(solid, program=\"grain = fbm(scale_x=5); albedo = grain\")\noutput textured",
    )?;
    text_case(
        "box = solid.box(size=[1,1,1])\ntriangles = poly.from_solid(box)\ncage = poly.merge_coplanar(triangles)\nsharp_edges = poly.select(cage, element=\"edge\", by=\"sharp\", angle=1.0)\ncorners = poly.select(cage, element=\"vertex\")\nsmooth = poly.subdivide(cage, creases=sharp_edges, corners=corners, levels=1, sharpness=2, corner_sharpness=2)\noutput smooth",
    )?;
    for sharpness in [0.0_f32, 0.5, 1.0, 2.0, 10.0] {
        text_case(&format!(
            "box = solid.box(size=[1,1,1])\ntriangles = poly.from_solid(box)\ncage = poly.merge_coplanar(triangles)\nsharp_edges = poly.select(cage, element=\"edge\", by=\"sharp\", angle=1.0)\ncorners = poly.select(cage, element=\"vertex\")\nsmooth = poly.subdivide(cage, creases=sharp_edges, corners=corners, levels=1, sharpness={sharpness}, corner_sharpness={sharpness})\noutput smooth"
        ))?;
    }
    Ok(())
}

/// Two-colour booleans: each colour's faces are a separate part, identical natively and in WASI.
fn colour_cases() -> Result<(), Box<dyn std::error::Error>> {
    text_case(
        "a = solid.box(size=[1,1,1])\nl = xform.move(a, by=[-0.4,0,0])\nr = xform.move(a, by=[0.4,0,0])\nred = solid.material(l, color=[0.9,0.1,0.1])\nblue = solid.material(r, color=[0.1,0.1,0.9])\nboth = solid.union(red, blue)\nmoved = xform.move(both, by=[0,2,0])\noutput moved",
    )?;
    text_case(
        "body = solid.box(size=[2,2,2])\nhole = solid.sphere(radius=0.8, segments=16)\nred = solid.material(body, color=[0.9,0.1,0.1])\ngreen = solid.material(hole, color=[0.1,0.8,0.1])\ncut = solid.difference(red, green)\noutput cut",
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    attribute_warning_case();
    subdivision_channel_case()?;
    surface_cases()?;
    opacity_case()?;
    field_cases()?;
    text_case(
        "b = solid.box(size=[1,1,1])\ns = inst.repeat(b, count=3, step=[2,0,0])\nm = inst.mirror(s, axis=\"x\")\noutput m",
    )?;
    text_case(
        "b = solid.box(size=[0.1,0.1,0.1])\ns = inst.scatter(b, count=40, width=4, depth=3, min_spacing=0.3, yaw_jitter=90deg, scale_jitter=0.3, seed=11)\nj = inst.jitter(s, shift=[0.05,0,0.05], yaw_jitter=15deg, scale_jitter=0.1, seed=3)\noutput j",
    )?;
    text_case(
        "b = solid.box(size=[2,4,6])\na = xform.align(b, from=[1,2,3], to=[-3,2,1], pivot=[2,1,0])\noutput a",
    )?;
    let mut session = Session::with_builtin(1);
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":0,
        "transaction":"t_parity","commands":[
        {"add_node":{"op":"solid.box@1","params":{"size":[1,1,1]},"alias":"$a"}},
        {"add_node":{"op":"solid.box@1","params":{"size":[1,1,1]},"alias":"$b"}},
        {"add_node":{"op":"xform.move@1","params":{"by":[0.5,0,0]},"alias":"$move"}},
        {"add_node":{"op":"solid.difference@1","alias":"$cut"}},
        {"connect":{"from":{"node":"$b","port":"out"},"to":{"node":"$move","port":"in"}}},
        {"connect":{"from":{"node":"$a","port":"out"},"to":{"node":"$cut","port":"in"}}},
        {"connect":{"from":{"node":"$move","port":"out"},"to":{"node":"$cut","port":"in"}}},
        {"set_output":{"name":"cut","from":{"node":"$cut","port":"out"}}}
    ]});
    if !matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ) {
        return Err("parity fixture rejected".into());
    }
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| format!("{}: {}", error.code, error.message))?;
    for part in result.parts {
        if (part.volume - 0.5).abs() > 1e-6 {
            return Err("analytic boolean volume mismatch".into());
        }
        println!(
            "{}",
            serde_json::json!({"node":part.node,"volume":part.volume,
            "positions":part.positions,"indices":part.indices})
        );
    }
    Ok(())
}
