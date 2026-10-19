
#[test]
fn a_socket_names_the_part_it_accepts() {
    let program = |params: &str| {
        evaluate(&format!(
            "k = solid.box@1(size=[0.2, 0.2, 0.2])\nkey = behavior.on@1(k, trigger=\"hold\", sets=\"carried\")\noutput key_out = key\n\
             l = solid.box@1(size=[0.3, 0.3, 0.05])\nlock = behavior.on@1(l{params})\noutput lock_out = lock\n"
        ))
    };
    let ok = program(", trigger=\"socket\", accepts=\"key_out\", radius=0.4, sets=\"unlocked\"")
        .unwrap();
    let lock = ok
        .parts
        .iter()
        .find(|p| p.name == "lock_out")
        .unwrap()
        .behavior
        .clone()
        .unwrap();
    assert_eq!(
        (lock.accepts.as_deref(), lock.radius),
        (Some("key_out"), 0.4)
    );
    assert!(
        program(", trigger=\"socket\", sets=\"unlocked\"")
            .err()
            .unwrap()
            .1
            .contains("accepts"),
        "a socket needs its part"
    );
    assert!(
        program(", trigger=\"select\", accepts=\"key_out\", sets=\"x\"")
            .err()
            .unwrap()
            .1
            .contains("only goes with"),
        "accepts means nothing elsewhere"
    );
}

#[test]
fn tracks_are_deterministic() {
    let a = evaluate(DOOR).unwrap();
    let b = evaluate(DOOR).unwrap();
    assert_eq!(a.parts[0].animation, b.parts[0].animation);
}
