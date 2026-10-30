//! `body.hair` with `fine` and `cover`: hair that reads as hair. No bare scalp shows through (every ray from the middle of the head out through the hair region hits hair),
//! the hair is made of fine strands instead of wide flat ribbons, never sinks into the skull, and carries UVs (u across the strand, v from root 0 to tip 1) so a texture can
//! paint root-to-tip colour and strand streaks. Written before the change; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality, set_human_pack};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    set_human_pack(&std::fs::read(path).unwrap()).unwrap();
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g30_hair").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

struct Scene {
    hair: ve_modeling::Part,
    body: ve_modeling::Part,
    head: [f32; 3],
}

fn grow(extra: &str) -> Scene {
    let source = format!(
        "body = body.human@1()\nhead_n = body.bone@1(body, bone=\"head\")\nh = body.hair@1(body{extra})\noutput head = head_n\noutput body_part = body\noutput hair_part = h\n"
    );
    let result = evaluate(&source).unwrap();
    let take = |name: &str| {
        result
            .parts
            .iter()
            .find(|p| p.name == name)
            .unwrap()
            .clone()
    };
    let bone = take("head").bone.unwrap();
    // the middle of the skull: a little above the head joint
    let head = [bone.head[0], bone.head[1] + 0.09, bone.head[2] + 0.01];
    Scene {
        hair: take("hair_part"),
        body: take("body_part"),
        head,
    }
}

const FINE: &str = ", fine=1, cover=true";

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Whether the ray from `origin` along `dir` hits any triangle of `part`.
fn hits(part: &ve_modeling::Part, origin: [f32; 3], dir: [f32; 3]) -> bool {
    part.indices.chunks(3).any(|t| {
        let (a, b, c) = (
            part.positions[t[0] as usize],
            part.positions[t[1] as usize],
            part.positions[t[2] as usize],
        );
        let (e1, e2) = (sub(b, a), sub(c, a));
        let p = cross(dir, e2);
        let det = dot(e1, p);
        if det.abs() < 1e-12 {
            return false;
        }
        let s = sub(origin, a);
        let u = dot(s, p) / det;
        let q = cross(s, e1);
        let v = dot(dir, q) / det;
        u >= 0.0 && v >= 0.0 && u + v <= 1.0 && dot(e2, q) / det > 0.0
    })
}

#[test]
fn no_bare_scalp_shows_through_the_hair() {
    let scene = grow(FINE);
    let mut missed = 0;
    let mut total = 0;
    // directions over the crown, the back and the sides above the ears (not the face)
    for ring in 0..9 {
        let elevation = 0.25 + ring as f32 * 0.17; // radians above the horizontal, up to the crown
        for k in 0..40 {
            let azimuth =
                std::f32::consts::PI * 0.62 + k as f32 / 40.0 * std::f32::consts::TAU * 0.62; // from over one ear round the back to the other
            let dir = [
                elevation.cos() * azimuth.sin(),
                elevation.sin(),
                -elevation.cos() * azimuth.cos(),
            ];
            total += 1;
            if !hits(&scene.hair, scene.head, dir) {
                missed += 1;
            }
        }
    }
    assert!(
        missed * 100 <= total,
        "{missed} of {total} rays through the crown and back found no hair"
    );
}

#[test]
fn hair_is_made_of_fine_strands_not_wide_ribbons() {
    let coarse = grow("");
    let fine = grow(FINE);
    let width = |p: &ve_modeling::Part| {
        // the shortest edge of each triangle is about the strip width at its widest end
        let mut w: Vec<f32> = p
            .indices
            .chunks(3)
            .map(|t| {
                let v = |i: u32| p.positions[i as usize];
                let d = |a: [f32; 3], b: [f32; 3]| dot(sub(a, b), sub(a, b)).sqrt();
                let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                d(a, b).min(d(b, c)).min(d(c, a))
            })
            .collect();
        w.sort_by(f32::total_cmp);
        w[w.len() / 2]
    };
    assert!(
        width(&fine.hair) < width(&coarse.hair) * 0.6,
        "fine strands are narrower: {} against {}",
        width(&fine.hair),
        width(&coarse.hair)
    );
    assert!(
        fine.hair.indices.len() / 3 < 400_000,
        "and still within budget: {} triangles",
        fine.hair.indices.len() / 3
    );
}

#[test]
fn hair_never_sinks_into_the_skull() {
    let scene = grow(FINE);
    for p in scene.hair.positions.iter().step_by(97) {
        let nearest = scene
            .body
            .positions
            .iter()
            .map(|q| dot(sub(*q, *p), sub(*q, *p)).sqrt())
            .fold(f32::MAX, f32::min);
        assert!(
            nearest > 0.0015,
            "hair vertex {p:?} is {nearest} m from the skin"
        );
    }
}

#[test]
fn hair_carries_uvs_that_run_from_root_to_tip() {
    let scene = grow(FINE);
    let hair = &scene.hair;
    assert_eq!(
        hair.uvs.len(),
        hair.indices.len(),
        "two floats per triangle corner"
    );
    assert!(
        hair.uvs
            .iter()
            .all(|uv| (-1e-4..=1.0001).contains(&uv[0]) && (-1e-4..=1.0001).contains(&uv[1])),
        "uvs lie in the unit square"
    );
    // v grows with the distance from the scalp: tips (v near 1) are further from the head middle than roots (v near 0)
    let dist = |i: usize| {
        let p = hair.positions[hair.indices[i] as usize];
        dot(sub(p, scene.head), sub(p, scene.head)).sqrt()
    };
    let (mut root, mut tip, mut nr, mut nt) = (0.0, 0.0, 0, 0);
    for i in 0..hair.indices.len() {
        if hair.uvs[i][1] < 0.1 {
            root += dist(i);
            nr += 1;
        } else if hair.uvs[i][1] > 0.9 {
            tip += dist(i);
            nt += 1;
        }
    }
    assert!(nr > 100 && nt > 100, "roots and tips exist: {nr}, {nt}");
    assert!(
        tip / nt as f32 > root / nr as f32 + 0.02,
        "v runs root to tip: roots {} tips {}",
        root / nr as f32,
        tip / nt as f32
    );
}

#[test]
fn hair_bakes_a_texture_and_the_defaults_are_unchanged() {
    let source = "body = body.human@1()\nh = body.hair@1(body, fine=1, cover=true)\n\
        m = solid.material@1(h, color=[0.2, 0.1, 0.05])\n\
        t = solid.texture@1(m, program=\"s = fbm(scale_x=300, scale_y=4, iterations=2); albedo = colorize(s, gradient=[0:#221008, 1:#6a4020])\", tile=0, size=\"256\")\noutput hair = t\n";
    let result = evaluate(source).unwrap();
    assert!(
        result.warnings.iter().all(|w| !w.code.contains("bake")),
        "{:?}",
        result.warnings
    );
    let plain = grow("");
    let off = grow(", fine=0, cover=false");
    assert_eq!(plain.hair.positions, off.hair.positions);
}
