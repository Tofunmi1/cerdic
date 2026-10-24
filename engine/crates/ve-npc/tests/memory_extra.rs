//! Edge cases for the deterministic memory implementation.
#![expect(clippy::float_cmp, reason = "tests assert exact contract behavior")]

use ve_npc::{Embedder, LexicalEmbedder, MemoryDraft, MemoryKind, MemoryStore, RetrievalWeights};

fn draft(text: &str, importance: f32) -> MemoryDraft {
    MemoryDraft::new(MemoryKind::Observation, text, importance)
}

#[test]
fn stop_words_and_minimal_stemming_are_applied() {
    let embedder = LexicalEmbedder::new(256);
    assert_eq!(embedder.embed("the and of"), vec![0.0; 256]);
    assert_eq!(embedder.embed("owes"), embedder.embed("owe"));
    assert_eq!(embedder.embed("dogs"), embedder.embed("dog"));
}

#[test]
fn bigrams_preserve_some_word_order_information() {
    let embedder = LexicalEmbedder::new(4096);
    let dog_man = embedder.embed("dog bites man");
    let man_dog = embedder.embed("man bites dog");
    let similarity = dot(&dog_man, &man_dog);
    assert!(
        similarity < 1.0 && similarity > 0.0,
        "similarity {similarity}"
    );
}

#[test]
fn rank_handles_zero_k_and_empty_stores() {
    let embedder = LexicalEmbedder::default();
    let store = MemoryStore::new(2);
    assert!(
        store
            .rank("anything", 0.0, 1, &RetrievalWeights::default(), &embedder)
            .is_empty()
    );
    let mut store = store;
    store.add(draft("anything", 0.5), 0.0, &embedder);
    assert!(
        store
            .rank("anything", 0.0, 0, &RetrievalWeights::default(), &embedder)
            .is_empty()
    );
}

#[test]
fn nan_importance_becomes_zero_and_capacity_one_forgets_the_lower_keep_value() {
    let embedder = LexicalEmbedder::default();
    let mut store = MemoryStore::new(1);
    let first = store.add(draft("first", f32::NAN), 0.0, &embedder);
    assert_eq!(store.get(first).expect("first memory").importance, 0.0);
    let second = store.add(draft("second", 0.5), 0.0, &embedder);
    assert!(store.get(first).is_none());
    assert!(store.get(second).is_some());
}

#[test]
fn forgetting_ties_choose_the_lower_id_and_pins_survive() {
    let embedder = LexicalEmbedder::default();
    let mut store = MemoryStore::new(2);
    let first = store.add(draft("first", 0.5), 0.0, &embedder);
    let second = store.add(draft("second", 0.5), 0.0, &embedder);
    store.add(draft("third", 0.9), 0.0, &embedder);
    assert!(store.get(first).is_none());
    assert!(store.get(second).is_some());

    let mut pinned = MemoryStore::new(1);
    let protected = pinned.add(draft("protected", 0.0), 0.0, &embedder);
    assert!(pinned.pin(protected));
    pinned.add(draft("new", 0.5), 0.0, &embedder);
    assert!(pinned.get(protected).is_some());
    assert_eq!(pinned.len(), 1);
}

#[test]
fn sharing_into_a_full_store_uses_the_same_forgetting_rule() {
    let embedder = LexicalEmbedder::default();
    let mut teller = MemoryStore::new(4);
    let story = teller.add(draft("gossip", 0.8), 0.0, &embedder);
    let mut receiver = MemoryStore::new(1);
    let weak = receiver.add(draft("weak", 0.1), 1.0, &embedder);
    let shared = teller
        .share(story, &mut receiver, "Ada", 2.0, &embedder, 1)
        .expect("share");
    assert!(receiver.get(weak).is_none());
    assert!(receiver.get(shared).is_some());
}

#[test]
fn unicode_save_round_trips_and_unknown_kinds_are_rejected() {
    let embedder = LexicalEmbedder::default();
    let mut store = MemoryStore::new(3);
    store.add(draft("雪の村 — café 🐈", 0.7), 42.0, &embedder);
    let json = store.to_json();
    assert_eq!(MemoryStore::from_json(&json).expect("load").to_json(), json);

    let invalid = json.replace("Observation", "UnknownKind");
    assert!(MemoryStore::from_json(&invalid).is_err());
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
