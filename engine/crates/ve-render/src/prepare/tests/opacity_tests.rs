use super::*;

#[test]
fn transparent_instances_are_isolated_after_opaque_batches() {
    let mut glass_a = instance(1, 1.0);
    glass_a.transparent = true;
    glass_a.sort_key = 20;
    let mut glass_b = instance(1, 2.0);
    glass_b.transparent = true;
    glass_b.sort_key = 21;
    let (data, batches) = group_into_batches(&[glass_a, instance(1, 0.0), glass_b]);
    assert_eq!(data.len(), 3);
    assert_eq!(batches.len(), 3);
    assert!(!batches[0].transparent);
    assert!(
        batches[1..]
            .iter()
            .all(|batch| batch.transparent && batch.instances.end - batch.instances.start == 1)
    );
}
