//! Contract tests for ve-nav step N2b (ADR 0063, lesson 6 from Unreal's Mass and its Smart Objects): things in the world that NPCs **claim a
//! slot of** (a chair, a counter, a forge, a bed), use, and **release**: find a free one nearby, claim, go to the slot, arrive and face the
//! right way, release, cool down. Claims that are never released (the NPC died) expire.
#![expect(
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "tests assert by failing and use small fixture coordinates"
)]

use ve_nav::{ClaimError, ObjectId, SlotDef, SmartObjectDef, SmartObjects};

fn bench(at: [f32; 3], slots: usize) -> SmartObjectDef {
    SmartObjectDef {
        kind: "bench".to_owned(),
        position: at,
        slots: (0..slots)
            .map(|i| SlotDef {
                offset: [i as f32 * 0.5, 0.0, 0.0],
                facing_deg: 90.0,
            })
            .collect(),
        cooldown_secs: 0.0,
        claim_ttl_secs: 60.0,
    }
}

fn forge(at: [f32; 3]) -> SmartObjectDef {
    SmartObjectDef {
        kind: "forge".to_owned(),
        position: at,
        slots: vec![SlotDef {
            offset: [0.0, 0.0, 1.0],
            facing_deg: 180.0,
        }],
        cooldown_secs: 5.0,
        claim_ttl_secs: 60.0,
    }
}

#[test]
fn finding_returns_free_objects_of_a_kind_nearest_first_within_range() {
    let mut world = SmartObjects::new();
    let far = world.add(bench([20.0, 0.0, 0.0], 1));
    let near = world.add(bench([2.0, 0.0, 0.0], 1));
    let mid = world.add(bench([5.0, 0.0, 0.0], 1));
    let _smith = world.add(forge([1.0, 0.0, 0.0]));
    let found = world.find("bench", [0.0, 0.0, 0.0], 10.0, 0.0);
    assert_eq!(
        found,
        vec![near, mid],
        "benches only, within 10 m, nearest first"
    );
    assert!(!found.contains(&far));
    assert_eq!(
        world.find("bench", [0.0, 0.0, 0.0], 100.0, 0.0),
        vec![near, mid, far]
    );
    assert!(world.find("throne", [0.0, 0.0, 0.0], 100.0, 0.0).is_empty());
}

#[test]
fn ties_in_distance_go_to_the_lower_id() {
    let mut world = SmartObjects::new();
    let a = world.add(bench([3.0, 0.0, 0.0], 1));
    let b = world.add(bench([-3.0, 0.0, 0.0], 1));
    assert_eq!(world.find("bench", [0.0, 0.0, 0.0], 10.0, 0.0), vec![a, b]);
}

#[test]
fn a_claim_takes_the_first_free_slot_and_reports_where_to_stand() {
    let mut world = SmartObjects::new();
    let id = world.add(bench([10.0, 0.0, 4.0], 2));
    let first = world.claim(id, "anna", 0.0).expect("free");
    assert_eq!(
        (first.object, first.slot, first.who.as_str()),
        (id, 0, "anna")
    );
    assert_eq!(
        first.position,
        [10.0, 0.0, 4.0],
        "object position plus the slot offset"
    );
    assert_eq!(first.facing_deg, 90.0);
    let second = world.claim(id, "ben", 0.0).expect("the second slot");
    assert_eq!(second.slot, 1);
    assert_eq!(second.position, [10.5, 0.0, 4.0]);
    assert_eq!(world.claim(id, "cara", 0.0), Err(ClaimError::Full));
    assert!(
        world.find("bench", [10.0, 0.0, 4.0], 5.0, 0.0).is_empty(),
        "no free slot, not offered"
    );
}

#[test]
fn one_npc_cannot_hold_two_slots_of_the_same_object() {
    let mut world = SmartObjects::new();
    let id = world.add(bench([0.0, 0.0, 0.0], 2));
    world.claim(id, "anna", 0.0).expect("free");
    assert_eq!(
        world.claim(id, "anna", 1.0),
        Err(ClaimError::AlreadyClaimed)
    );
}

#[test]
fn releasing_frees_the_slot_and_a_cooldown_keeps_it_off_the_market_for_a_while() {
    let mut world = SmartObjects::new();
    let id = world.add(forge([0.0, 0.0, 0.0]));
    let claim = world.claim(id, "smith", 0.0).expect("free");
    assert_eq!(world.claim(id, "boy", 1.0), Err(ClaimError::Full));
    world.release(&claim, 10.0).expect("releases");
    assert_eq!(
        world.claim(id, "boy", 12.0),
        Err(ClaimError::Cooldown),
        "5 s cooldown after release at 10"
    );
    assert!(
        world.find("forge", [0.0, 0.0, 0.0], 5.0, 12.0).is_empty(),
        "cooling slots are not offered either"
    );
    assert_eq!(world.find("forge", [0.0, 0.0, 0.0], 5.0, 15.5), vec![id]);
    assert!(world.claim(id, "boy", 15.5).is_ok());
}

#[test]
fn releasing_twice_or_something_never_claimed_is_an_error() {
    let mut world = SmartObjects::new();
    let id = world.add(bench([0.0, 0.0, 0.0], 1));
    let claim = world.claim(id, "anna", 0.0).expect("free");
    world.release(&claim, 1.0).expect("releases");
    assert_eq!(world.release(&claim, 2.0), Err(ClaimError::NotClaimed));
    assert_eq!(
        world.claim(ObjectId(999), "anna", 0.0),
        Err(ClaimError::UnknownObject)
    );
    let mut other = SmartObjects::new();
    let other_id = other.add(bench([0.0, 0.0, 0.0], 1));
    let foreign = other.claim(other_id, "zed", 0.0).expect("free");
    assert!(
        world.release(&foreign, 0.0).is_err(),
        "a claim from another world is not ours to release"
    );
}

#[test]
fn claims_that_are_never_released_expire_after_their_time_to_live() {
    let mut world = SmartObjects::new();
    let id = world.add(bench([0.0, 0.0, 0.0], 1));
    let claim = world.claim(id, "anna", 0.0).expect("free");
    assert!(world.tick(59.0).is_empty());
    let expired = world.tick(61.0);
    assert_eq!(
        expired,
        vec![claim.clone()],
        "the stale claim is reported so the caller can tell the NPC"
    );
    assert!(
        world.claim(id, "ben", 61.0).is_ok(),
        "the slot is free again"
    );
    assert_eq!(
        world.release(&claim, 62.0),
        Err(ClaimError::NotClaimed),
        "the old holder's release no longer counts"
    );
}

#[test]
fn what_an_npc_holds_can_be_listed_and_removing_an_object_drops_its_claims() {
    let mut world = SmartObjects::new();
    let a = world.add(bench([0.0, 0.0, 0.0], 1));
    let b = world.add(forge([4.0, 0.0, 0.0]));
    world.claim(a, "anna", 0.0).expect("free");
    world.claim(b, "anna", 0.0).expect("free");
    let held = world.claimed_by("anna");
    assert_eq!(
        held.iter().map(|c| c.object).collect::<Vec<_>>(),
        vec![a, b],
        "in object order"
    );
    assert!(world.claimed_by("nobody").is_empty());
    world.remove(a);
    assert_eq!(world.claimed_by("anna").len(), 1);
    assert_eq!(world.claim(a, "ben", 0.0), Err(ClaimError::UnknownObject));
}

#[test]
fn the_same_sequence_of_calls_always_gives_the_same_result() {
    let run = || {
        let mut world = SmartObjects::new();
        let ids: Vec<_> = (0..5)
            .map(|i| world.add(bench([i as f32 * 2.0, 0.0, 0.0], 2)))
            .collect();
        let mut log = Vec::new();
        for (n, who) in ["a", "b", "c", "d", "e", "f"].iter().enumerate() {
            let near = world.find("bench", [0.0, 0.0, 0.0], 100.0, n as f64);
            let claim = world.claim(near[0], who, n as f64).expect("a free bench");
            log.push((claim.object, claim.slot));
        }
        (ids, log)
    };
    assert_eq!(run(), run());
}
