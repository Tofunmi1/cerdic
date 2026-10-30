//! Contract tests for NPC memory (ADR 0064, step M1): an episodic stream retrieved by recency, importance and relevance, facts, relationships,
//! gossip with provenance, a reflection trigger, a capacity that forgets the least valuable, and a saved form.
#![expect(
    clippy::float_cmp,
    clippy::many_single_char_names,
    clippy::cast_precision_loss,
    reason = "tests assert exact values the contract fixes and name small fixtures a, b, c"
)]

use ve_npc::{
    Embedder, LexicalEmbedder, MemoryDraft, MemoryError, MemoryKind, MemoryStore, RetrievalWeights,
    Source,
};

fn embedder() -> LexicalEmbedder {
    LexicalEmbedder::new(256)
}

fn draft(text: &str, importance: f32) -> MemoryDraft {
    MemoryDraft::new(MemoryKind::Observation, text, importance)
}

fn only(recency: f32, importance: f32, relevance: f32) -> RetrievalWeights {
    RetrievalWeights {
        recency,
        importance,
        relevance,
        half_life: 3600.0,
    }
}

#[test]
fn the_lexical_embedder_is_deterministic_unit_length_and_safe_on_nothing() {
    let e = embedder();
    let a = e.embed("The baker gave me fresh bread");
    let b = e.embed("The baker gave me fresh bread");
    assert_eq!(a, b);
    assert_eq!(a.len(), e.dim());
    let norm = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-4, "norm {norm}");
    let empty = e.embed("");
    assert_eq!(empty.len(), e.dim());
    assert!(empty.iter().all(|x| *x == 0.0), "no words, no direction");
    let stop_only = e.embed("the of and a");
    assert!(
        stop_only.iter().all(|x| *x == 0.0),
        "stop words carry no meaning"
    );
}

#[test]
fn relevance_finds_the_memory_about_the_question() {
    let e = embedder();
    let mut store = MemoryStore::new(64);
    let baker = store.add(
        draft("the baker gave me fresh bread this morning", 0.5),
        0.0,
        &e,
    );
    let wolves = store.add(draft("wolves howled on the hill at night", 0.5), 0.0, &e);
    let smith = store.add(draft("the smith sharpened my sword", 0.5), 0.0, &e);
    let ranked = store.rank("who gave me bread", 0.0, 3, &only(0.0, 0.0, 1.0), &e);
    assert_eq!(ranked[0].id, baker);
    assert!(ranked[0].relevance > 0.0);
    assert!(ranked[0].relevance > ranked[1].relevance);
    assert!(ranked.iter().any(|r| r.id == wolves) && ranked.iter().any(|r| r.id == smith));
}

#[test]
fn recency_halves_every_half_life_since_last_access() {
    let e = embedder();
    let mut store = MemoryStore::new(64);
    let old = store.add(draft("same words", 0.5), 0.0, &e);
    let new = store.add(draft("same words", 0.5), 3600.0, &e);
    let ranked = store.rank("anything", 3600.0, 2, &only(1.0, 0.0, 0.0), &e);
    assert_eq!(ranked[0].id, new);
    assert!((ranked[0].recency - 1.0).abs() < 1e-4);
    assert_eq!(ranked[1].id, old);
    assert!(
        (ranked[1].recency - 0.5).abs() < 1e-4,
        "{}",
        ranked[1].recency
    );
    let later = store.rank("anything", 7200.0, 2, &only(1.0, 0.0, 0.0), &e);
    assert!(
        (later[1].recency - 0.25).abs() < 1e-4,
        "{}",
        later[1].recency
    );
}

#[test]
fn importance_orders_otherwise_equal_memories() {
    let e = embedder();
    let mut store = MemoryStore::new(64);
    store.add(draft("a quiet walk", 0.2), 0.0, &e);
    let big = store.add(draft("the village burned", 0.95), 0.0, &e);
    store.add(draft("a small meal", 0.4), 0.0, &e);
    let ranked = store.rank("x", 0.0, 3, &only(0.0, 1.0, 0.0), &e);
    assert_eq!(ranked[0].id, big);
    assert!(
        ranked[0].importance > ranked[1].importance && ranked[1].importance > ranked[2].importance
    );
}

#[test]
fn the_score_is_the_weighted_sum_of_the_three_parts() {
    let e = embedder();
    let mut store = MemoryStore::new(64);
    store.add(draft("the baker gave me fresh bread", 0.6), 0.0, &e);
    let w = only(2.0, 3.0, 5.0);
    let r = store.rank("bread from the baker", 1800.0, 1, &w, &e);
    let expect = 2.0 * r[0].recency + 3.0 * r[0].importance + 5.0 * r[0].relevance;
    assert!(
        (r[0].score - expect).abs() < 1e-4,
        "{} vs {expect}",
        r[0].score
    );
}

#[test]
fn equal_scores_go_to_the_lower_id_every_time() {
    let e = embedder();
    let mut store = MemoryStore::new(64);
    let first = store.add(draft("identical", 0.5), 0.0, &e);
    let second = store.add(draft("identical", 0.5), 0.0, &e);
    for _ in 0..20 {
        let r = store.rank("identical", 0.0, 2, &RetrievalWeights::default(), &e);
        assert_eq!(r[0].id, first);
        assert_eq!(r[1].id, second);
    }
}

#[test]
fn retrieving_touches_what_it_returns_but_ranking_does_not() {
    let e = embedder();
    let mut store = MemoryStore::new(64);
    let id = store.add(draft("the baker gave me bread", 0.5), 0.0, &e);
    let _ = store.rank("bread", 50.0, 1, &RetrievalWeights::default(), &e);
    let m = store.get(id).expect("memory");
    assert_eq!((m.access_count, m.last_access), (0, 0.0));
    let got = store.retrieve("bread", 100.0, 1, &RetrievalWeights::default(), &e);
    assert_eq!(got[0].id, id);
    let m = store.get(id).expect("memory");
    assert_eq!((m.access_count, m.last_access), (1, 100.0));
}

#[test]
fn importance_is_clamped_and_embeddings_match_the_embedder() {
    let e = embedder();
    let mut store = MemoryStore::new(64);
    let high = store.add(draft("huge", 5.0), 0.0, &e);
    let low = store.add(draft("tiny", -1.0), 0.0, &e);
    assert_eq!(store.get(high).expect("m").importance, 1.0);
    assert_eq!(store.get(low).expect("m").importance, 0.0);
    assert_eq!(store.get(high).expect("m").embedding.len(), e.dim());
}

#[test]
fn recent_lists_newest_first() {
    let e = embedder();
    let mut store = MemoryStore::new(64);
    store.add(draft("one", 0.5), 1.0, &e);
    store.add(draft("two", 0.5), 2.0, &e);
    store.add(draft("three", 0.5), 3.0, &e);
    let texts: Vec<_> = store.recent(2).iter().map(|m| m.text.clone()).collect();
    assert_eq!(texts, vec!["three", "two"]);
}

#[test]
fn the_least_valuable_unpinned_memory_is_forgotten_over_capacity() {
    let e = embedder();
    let mut store = MemoryStore::new(3);
    let keep = store.add(draft("precious but pinned", 0.1), 0.0, &e);
    store.pin(keep);
    store.add(draft("important", 0.9), 0.0, &e);
    let dull = store.add(draft("dull", 0.2), 0.0, &e);
    store.add(draft("fairly important", 0.7), 0.0, &e);
    assert_eq!(store.len(), 3);
    assert!(
        store.get(keep).is_some(),
        "pinned survives however unimportant"
    );
    assert!(store.get(dull).is_none(), "the dullest unpinned goes first");
}

#[test]
fn facts_overwrite_and_return_what_they_replaced() {
    let mut store = MemoryStore::new(8);
    assert!(store.set_fact("player.name", "Ada", 0.9, 1.0).is_none());
    let old = store
        .set_fact("player.name", "Adaline", 0.8, 2.0)
        .expect("replaced");
    assert_eq!(old.value, "Ada");
    let now = store.fact("player.name").expect("fact");
    assert_eq!(
        (now.value.as_str(), now.confidence, now.time),
        ("Adaline", 0.8, 2.0)
    );
    assert!(store.fact("missing").is_none());
}

#[test]
fn relationships_move_and_stay_inside_their_ranges() {
    let mut store = MemoryStore::new(8);
    let r = store.relationship("player");
    assert_eq!((r.affinity, r.trust, r.familiarity), (0.0, 0.0, 0.0));
    store.adjust_relationship("player", 0.7, 0.3, 0.6);
    store.adjust_relationship("player", 0.7, -2.0, 0.6);
    let r = store.relationship("player");
    assert_eq!(r.affinity, 1.0);
    assert_eq!(r.trust, -1.0);
    assert_eq!(r.familiarity, 1.0);
    store.adjust_relationship("player", -5.0, 0.0, -5.0);
    let r = store.relationship("player");
    assert_eq!((r.affinity, r.familiarity), (-1.0, 0.0));
}

#[test]
fn gossip_carries_provenance_loses_weight_and_stops_after_the_hop_limit() {
    let e = embedder();
    let (mut a, mut b, mut c) = (
        MemoryStore::new(8),
        MemoryStore::new(8),
        MemoryStore::new(8),
    );
    let original = a.add(draft("the miller owes the baker money", 0.8), 10.0, &e);
    let at_b = a
        .share(original, &mut b, "Anna", 20.0, &e, 2)
        .expect("told");
    let m = b.get(at_b).expect("memory");
    assert_eq!(
        m.source,
        Source::Told {
            from: "Anna".to_owned(),
            hops: 1
        }
    );
    assert!(
        (m.importance - 0.64).abs() < 1e-4,
        "importance times 0.8: {}",
        m.importance
    );
    assert_eq!(m.text, "the miller owes the baker money");
    assert_eq!(
        a.get(original).expect("kept").importance,
        0.8,
        "the teller keeps its own memory unchanged"
    );
    let at_c = b
        .share(at_b, &mut c, "Ben", 30.0, &e, 2)
        .expect("told again");
    assert_eq!(
        c.get(at_c).expect("m").source,
        Source::Told {
            from: "Ben".to_owned(),
            hops: 2
        }
    );
    let (mut d, mut stop) = (MemoryStore::new(8), MemoryStore::new(8));
    let at_d = b.share(at_b, &mut d, "Ben", 31.0, &e, 1);
    assert!(
        matches!(at_d, Err(MemoryError::TooManyHops { .. })),
        "{at_d:?}"
    );
    assert!(matches!(
        a.share(ve_npc::MemoryId(999), &mut stop, "Anna", 0.0, &e, 2),
        Err(MemoryError::UnknownMemory(_))
    ));
}

#[test]
fn reflection_is_due_when_enough_importance_has_piled_up() {
    let e = embedder();
    let mut store = MemoryStore::new(16);
    store.add(draft("a", 0.3), 1.0, &e);
    store.add(draft("b", 0.4), 2.0, &e);
    store.add(draft("c", 0.5), 3.0, &e);
    assert!((store.importance_since_reflection() - 1.2).abs() < 1e-4);
    assert!(store.reflection_due(1.0));
    assert!(!store.reflection_due(1.5));
    store.mark_reflected(4.0);
    assert!(store.importance_since_reflection().abs() < 1e-6);
    assert!(!store.reflection_due(1.0));
    store.add(draft("d", 0.2), 5.0, &e);
    assert!((store.importance_since_reflection() - 0.2).abs() < 1e-4);
}

#[test]
fn a_store_saves_and_loads_without_losing_anything() {
    let e = embedder();
    let mut store = MemoryStore::new(16);
    store.add(draft("the baker gave me bread", 0.6), 1.0, &e);
    let pinned = store.add(
        MemoryDraft::new(MemoryKind::Reflection, "I trust the baker", 0.9),
        2.0,
        &e,
    );
    store.pin(pinned);
    store.set_fact("baker.mood", "cheerful", 0.7, 3.0);
    store.adjust_relationship("baker", 0.4, 0.2, 0.1);
    let _ = store.retrieve("bread", 5.0, 1, &RetrievalWeights::default(), &e);
    let json = store.to_json();
    let mut loaded = MemoryStore::from_json(&json).expect("loads");
    assert_eq!(loaded.to_json(), json, "save, load, save is stable");
    let w = RetrievalWeights::default();
    let a: Vec<_> = store
        .rank("bread", 9.0, 2, &w, &e)
        .iter()
        .map(|r| (r.id, r.score))
        .collect();
    let b: Vec<_> = loaded
        .rank("bread", 9.0, 2, &w, &e)
        .iter()
        .map(|r| (r.id, r.score))
        .collect();
    assert_eq!(a, b);
    assert_eq!(loaded.fact("baker.mood").expect("fact").value, "cheerful");
    assert_eq!(loaded.relationship("baker").affinity, 0.4);
    let next = loaded.add(draft("new", 0.5), 10.0, &e);
    assert!(next.0 > pinned.0, "ids keep counting after a load");
    assert!(matches!(
        MemoryStore::from_json("{not json"),
        Err(MemoryError::BadSave(_))
    ));
}

#[test]
fn two_stores_built_the_same_way_are_identical() {
    let e = embedder();
    let build = || {
        let mut s = MemoryStore::new(8);
        for (i, t) in ["bread", "wolves", "sword", "rain"].iter().enumerate() {
            s.add(draft(t, 0.1 * i as f32), i as f64, &e);
        }
        s
    };
    assert_eq!(build().to_json(), build().to_json());
}
