#[test]
fn a_chain_of_edits_is_not_refused_by_the_plan_budget() {
    // Written by a model (Sonnet) from a prose request; an earlier size estimate multiplied along the
    // chain (inset, extrude, bevel) and refused it as 426,176 triangles against a 200,000 limit.
    let program = "box = solid.box@1(size=[1, 1, 1])\n\
        tris = poly.from_solid@1(box)\n\
        flat = poly.merge_coplanar@1(tris)\n\
        top = poly.select@1(flat, by=\"normal\", angle=0.0008726646259971648rad)\n\
        inset = poly.inset_region@1(flat, top, thickness=0.15)\n\
        inner = poly.select@1(inset, by=\"box\", box_min=[-0.2, 0.4, -0.2], box_max=[0.2, 0.6, 0.2])\n\
        raised = poly.extrude_region@1(inset, inner, offset=[0, 0.1, 0])\n\
        hard = poly.select@1(raised, element=\"edge\", by=\"sharp\", angle=0.008726646259971648rad)\n\
        rounded = poly.bevel_network@1(raised, hard, width=0.02)\n\
        block = poly.to_solid@1(rounded)\n\
        output block";
    let v = volume(program);
    // 1 + 0.7 x 0.7 x 0.1 = 1.049, less a little from the rounded edges.
    assert!(v > 1.04 && v < 1.049, "volume {v}");
    let document = session(program);
    for quality in [Quality::Preview, Quality::Final] {
        let result = Evaluator::default().evaluate(document.document(), quality);
        assert!(
            result.is_ok(),
            "{quality:?}: {:?}",
            result.err().map(|e| e.code)
        );
    }
}

fn triangles(text: &str) -> usize {
    evaluate(text)
        .unwrap_or_else(|(code, message)| panic!("{code}: {message}"))
        .parts[0]
        .indices
        .len()
        / 3
}

#[test]
fn decimating_a_sphere_keeps_its_shape_with_a_quarter_of_the_triangles() {
    let sphere = "ball = solid.sphere@1(radius=1, segments=64)\n\
         cage = poly.from_solid@1(ball)\n";
    let full = format!("{sphere}back = poly.to_solid@1(cage)\noutput back");
    let reduced = format!(
        "{sphere}small = poly.decimate@1(cage, ratio=0.25)\n\
         back = poly.to_solid@1(small)\n\
         output back"
    );
    let (full_volume, full_triangles) = (volume(&full), triangles(&full));
    let (small_volume, small_triangles) = (volume(&reduced), triangles(&reduced));
    let kept = small_triangles as f64 / full_triangles as f64;
    assert!(
        (0.2..0.35).contains(&kept),
        "kept {kept} of {full_triangles}"
    );
    // Collapsing the flattest edges first loses very little of the sphere's volume.
    assert!(
        (small_volume / full_volume - 1.0).abs() < 0.05,
        "{small_volume} vs {full_volume}"
    );
}

#[test]
fn unsubdividing_a_smoothed_cube_returns_a_coarse_cage() {
    let coarse = triangles(&format!(
        "{BOX}smooth = poly.subdivide@1(mesh, levels=2)\n\
         cage = poly.unsubdivide@1(smooth, iterations=2)\n\
         back = poly.to_solid@1(cage)\n\
         output back"
    ));
    let dense = triangles(&format!(
        "{BOX}smooth = poly.subdivide@1(mesh, levels=2)\n\
         back = poly.to_solid@1(smooth)\n\
         output back"
    ));
    assert!(
        coarse * 4 <= dense,
        "{coarse} triangles after unsubdividing, {dense} before"
    );
}

#[test]
fn a_wireframe_is_a_thin_frame_of_the_box() {
    let v = volume(&format!(
        "{BOX}cage = poly.wireframe@1(mesh, thickness=0.05)\n\
         solid = poly.to_solid@1(cage)\n\
         output solid"
    ));
    assert!(v.abs() > 0.0 && v.abs() < 0.6, "volume {v}");
}

#[test]
fn wrong_selection_kinds_and_missing_selections_refuse() {
    let edges = refused(&format!(
        "{BOX}sharp = poly.select@1(mesh, element=\"edge\", by=\"sharp\", angle=0.5)\n\
         inset = poly.inset_region@1(mesh, sharp, thickness=0.1)\n\
         solid = poly.to_solid@1(inset)\n\
         output solid"
    ));
    assert_eq!(edges, "poly.selection_kind");
    let faces = refused(&format!(
        "{BOX}{TOP}\
         more = poly.subdivide_edges@1(mesh, top, cuts=1)\n\
         solid = poly.to_solid@1(more)\n\
         output solid"
    ));
    assert_eq!(faces, "poly.selection_kind");
}

#[test]
fn the_results_are_deterministic() {
    let program = format!(
        "{BOX}{TOP}\
         inset = poly.inset_region@1(mesh, top, thickness=0.15, depth=0.1)\n\
         inner = poly.select@1(inset, by=\"box\", box_min=[-0.2, 0.4, -0.2], box_max=[0.2, 0.7, 0.2])\n\
         raised = poly.extrude_region@1(inset, inner, offset=[0, 0.2, 0])\n\
         cut = poly.dissolve_limit@1(raised, angle=0.05)\n\
         solid = poly.to_solid@1(cut)\n\
         output solid"
    );
    let first = evaluate(&program).unwrap();
    let second = evaluate(&program).unwrap();
    assert_eq!(
        first.parts[0].volume.to_bits(),
        second.parts[0].volume.to_bits()
    );
    assert_eq!(first.parts[0].indices, second.parts[0].indices);
}

#[test]
fn packing_unwrapped_uvs_is_deterministic_and_bounded() {
    let program = "box = solid.box@1(size=[1, 1, 1])\n\
        tris = poly.from_solid@1(box)\n\
        uv = poly.unwrap_uv@1(tris, pack=false)\n\
        packed = poly.pack_uv@1(uv, margin=0.02, rotate=false)\n\
        solid = poly.to_solid@1(packed)\n\
        output solid";
    let first = evaluate(program).unwrap();
    let second = evaluate(program).unwrap();
    let a = &first.parts[0].uvs;
    let b = &second.parts[0].uvs;
    assert!(a.iter().flatten().all(|uv| (0.0..=1.0).contains(uv)));
    assert_eq!(
        a.iter().flatten().map(|v| v.to_bits()).collect::<Vec<_>>(),
        b.iter().flatten().map(|v| v.to_bits()).collect::<Vec<_>>()
    );
    let missing = evaluate(
        "box = solid.box@1(size=[1,1,1])\ntris = poly.from_solid@1(box)\npacked = poly.pack_uv@1(tris)\nsolid = poly.to_solid@1(packed)\noutput solid",
    );
    let (_, message) = missing.unwrap_err();
    assert!(message.contains("unwrap_uv"), "{message}");
}

#[test]
fn lod_levels_reduce_a_sphere_progressively_and_reject_collapse() {
    let base = "ball = solid.sphere@1(radius=1, segments=48)\nmesh = poly.from_solid@1(ball)\n";
    let counts: Vec<_> = (0..=3).map(|level| triangles(&format!("{base}lod = poly.lod@1(mesh, levels=3, level={level}, ratio=0.6)\nsolid = poly.to_solid@1(lod)\noutput solid"))).collect();
    assert!(
        counts.windows(2).all(|pair| pair[1] < pair[0]),
        "{counts:?}"
    );
    assert!((counts[1] as f64 / counts[0] as f64 - 0.6).abs() < 0.1);
    let err = evaluate(&format!("{base}lod = poly.lod@1(mesh, levels=4, level=4, ratio=0.05)\nsolid = poly.to_solid@1(lod)\noutput solid")).unwrap_err();
    assert!(err.1.contains("higher ratio"), "{}", err.1);
}

#[test]
fn lod_refuses_unavailable_protection_instead_of_ignoring_it() {
    let base = "ball = solid.sphere@1(radius=1, segments=16)\nmesh = poly.from_solid@1(ball)\n";
    for options in ["protect_uv_seams=true", "protect_boundary=true"] {
        let error = evaluate(&format!(
            "{base}lod = poly.lod@1(mesh, levels=2, level=1, ratio=0.6, {options})\nsolid = poly.to_solid@1(lod)\noutput solid"
        ))
        .unwrap_err();
        assert_eq!(error.0, "poly.lod_protection_unsupported");
        assert!(error.1.contains("cannot protect"), "{}", error.1);
    }
}
