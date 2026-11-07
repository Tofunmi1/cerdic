//! Prints the human pack's skeleton (clean name, parent) so the autorig macro can be checked against it. Run with --nocapture.
#![expect(clippy::unwrap_used, clippy::print_stderr, reason = "fixture")]

#[test]
fn dump_the_skeleton() {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/human/human.pack"
    ))
    .unwrap();
    let human = ve_human::Human::new(ve_human::Pack::from_zstd(&bytes).unwrap());
    let names = human.bone_names();
    let parents = human.bone_parents();
    for (i, name) in names.iter().enumerate() {
        let clean = name.to_lowercase().replace(['.', '-'], "_");
        let parent = usize::try_from(parents[i])
            .ok()
            .map(|p| names[p].to_lowercase().replace(['.', '-'], "_"));
        eprintln!(
            "BONE {i} {clean} <- {}",
            parent.unwrap_or_else(|| "-".to_owned())
        );
    }
}
