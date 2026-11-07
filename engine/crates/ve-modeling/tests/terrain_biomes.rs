//! `terrain.biomes`: biome layers that drive `scatter.place`. Do not weaken.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| ("parse".to_owned(), format!("{e:?}")))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_biomes").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

const LAND: &str = "land = terrain.tile@1(size=[160, 160], cell=2, seed=3, height=140, scale=90)\nplant = solid.box@1(size=[0.4, 1, 0.4])\n";

fn copies(result: &Evaluation, name: &str) -> Vec<[f32; 3]> {
    result
        .parts
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| p.transforms.as_ref())
        .map(|t| t.iter().map(|m| [m[12], m[13], m[14]]).collect())
        .unwrap_or_default()
}

#[test]
fn a_biome_layer_decides_where_plants_grow_and_a_narrower_window_plants_fewer() {
    let program = |rules: &str| {
        format!(
            "{LAND}b = terrain.biomes@1(land, rules=\"{rules}\")\nplants = scatter.place@1(plant, b, layer=\"biome_wood\", radius=2.5, seed=1)\noutput plants = plants\n"
        )
    };
    let everywhere = copies(
        &evaluate(&program("wood: slope 0..")).unwrap_or_else(|e| panic!("{e:?}")),
        "plants",
    );
    let gentle = copies(&evaluate(&program("wood: slope 0..12")).unwrap(), "plants");
    let steep = copies(&evaluate(&program("wood: slope 30..")).unwrap(), "plants");
    assert!(
        everywhere.len() > 300,
        "plants grow where the biome is: {}",
        everywhere.len()
    );
    assert!(
        gentle.len() < everywhere.len() && steep.len() < everywhere.len(),
        "a narrower window plants fewer: {} {} of {}",
        gentle.len(),
        steep.len(),
        everywhere.len()
    );
    assert!(
        gentle.len() > 30,
        "there is gentle ground: {}",
        gentle.len()
    );
    // Every plant of the gentle biome stands where the whole-land scatter also put one: the biome only thins the same arrangement.
    let key = |p: &[f32; 3]| ((p[0] * 10.0).round() as i64, (p[2] * 10.0).round() as i64);
    let all: std::collections::BTreeSet<_> = everywhere.iter().map(key).collect();
    assert!(
        gentle.iter().all(|p| all.contains(&key(p))),
        "the biome thins the same arrangement"
    );
}

#[test]
fn two_biomes_blend_so_their_plants_share_the_border() {
    let program = "land = terrain.tile@1(size=[160, 160], cell=2, seed=3, height=140, scale=90)\nplant = solid.box@1(size=[0.4, 1, 0.4])\n\
b = terrain.biomes@1(land, rules=\"low: height ..60; high: height 40..\", blend=1)\n\
lows = scatter.place@1(plant, b, layer=\"biome_low\", radius=2.5, seed=1)\nhighs = scatter.place@1(plant, b, layer=\"biome_high\", radius=2.5, seed=2)\n\
output lows = lows\noutput highs = highs\n";
    let result = evaluate(program).unwrap_or_else(|e| panic!("{e:?}"));
    let (lows, highs) = (copies(&result, "lows"), copies(&result, "highs"));
    assert!(!lows.is_empty() && !highs.is_empty());
    // The windows overlap between 40 and 60 m, so both kinds grow there (a hard cut would leave no height where both do).
    let band = |v: &[[f32; 3]]| v.iter().filter(|p| p[1] > 42.0 && p[1] < 58.0).count();
    assert!(
        band(&lows) > 3 && band(&highs) > 3,
        "both kinds grow in the overlap: {} and {}",
        band(&lows),
        band(&highs)
    );
    let mean = |v: &[[f32; 3]]| v.iter().map(|p| p[1]).sum::<f32>() / v.len() as f32;
    assert!(
        mean(&lows) < mean(&highs),
        "low plants sit lower: {} against {}",
        mean(&lows),
        mean(&highs)
    );
}

#[test]
fn a_bad_rule_is_refused_with_the_fix_and_neighbouring_tiles_get_the_same_plants_in_the_same_places()
 {
    let error = evaluate(&format!(
        "{LAND}b = terrain.biomes@1(land, rules=\"wood: steep 0..1\")\noutput land = b\n"
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "biome.rules");
    assert!(error.1.contains("unknown measure"), "{}", error.1);
    // A tile and the tile next to it, scattered with the same biome: no plant of one lies inside the other's area, and the same seed repeats.
    let tile = |x: i32| {
        format!(
            "land = terrain.tile@1(size=[80, 80], cell=2, tile_x={x}, seed=3, height=140, scale=90)\nplant = solid.box@1(size=[0.4, 1, 0.4])\nb = terrain.biomes@1(land, rules=\"wood: slope 0..\")\nplants = scatter.place@1(plant, b, layer=\"biome_wood\", radius=3, seed=1)\noutput plants = plants\n"
        )
    };
    let (a, b) = (
        copies(&evaluate(&tile(0)).unwrap(), "plants"),
        copies(&evaluate(&tile(1)).unwrap(), "plants"),
    );
    assert!(
        a.iter().all(|p| p[0] >= 0.0 && p[0] <= 80.0)
            && b.iter().all(|p| p[0] >= 80.0 && p[0] <= 160.0)
    );
    assert_eq!(a, copies(&evaluate(&tile(0)).unwrap(), "plants"));
    assert!(a.len() > 50 && b.len() > 50);
}
