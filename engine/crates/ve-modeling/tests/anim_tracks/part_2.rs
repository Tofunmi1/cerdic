
#[test]
fn a_wave_becomes_one_looping_track_that_starts_at_its_base() {
    let program = "b = solid.box@1(size=[1, 1, 1])\n\
        w = anim.wave@1(b, property=\"position\", amplitude=[0, 0.15, 0], period=2)\noutput o = w\n";
    let part = &evaluate(program).unwrap().parts[0];
    let [track] = &part.animation[..] else {
        panic!("one track")
    };
    assert!(track.looped);
    assert_eq!(track.keys.len(), 33, "32 samples and the closing key");
    assert!((track.duration() - 2.0).abs() < 1e-5);
    assert!(track.sample(0.0)[1].abs() < 1e-5, "starts at the middle");
    assert!(
        (track.sample(0.5)[1] - 0.15).abs() < 1e-3,
        "peaks a quarter period in"
    );
    assert!(
        (track.sample(1.5)[1] + 0.15).abs() < 1e-3,
        "bottoms out three quarters in"
    );
    assert!(
        (track.sample(2.0)[1] - track.sample(0.0)[1]).abs() < 1e-5,
        "closes its loop"
    );

    let pulse = "b = solid.box@1(size=[1, 1, 1])\n\
        w = anim.wave@1(b, property=\"scale\", shape=\"triangle\", amplitude=[0.2, 0.2, 0.2], period=1)\noutput o = w\n";
    let part = &evaluate(pulse).unwrap().parts[0];
    assert!(
        (part.animation[0].sample(0.25)[0] - 1.2).abs() < 1e-3,
        "scale centres on 1 by default"
    );
    let shifted = "b = solid.box@1(size=[1, 1, 1])\n\
        w = anim.wave@1(b, property=\"position\", amplitude=[1, 0, 0], period=4, phase=0.25)\noutput o = w\n";
    let part = &evaluate(shifted).unwrap().parts[0];
    assert!(
        (part.animation[0].sample(0.0)[0] - 1.0).abs() < 1e-3,
        "a quarter-period phase starts at the peak"
    );
}

#[test]
fn visibility_tracks_take_one_number_and_easing_options_ease() {
    let part = &evaluate(
        "b = solid.box@1(size=[1, 1, 1])\n\
         v = anim.track@1(b, property=\"visible\", keys=\"0:1; 1:0\", interpolation=\"step\", loop=false)\noutput o = v\n",
    )
    .unwrap()
    .parts[0];
    assert_eq!(part.animation[0].property, TrackProperty::Visible);
    assert!(
        part.animation[0].sample(0.5)[0] >= 0.5,
        "still shown before the second key"
    );
    assert!(
        part.animation[0].sample(1.0)[0] < 0.5,
        "hidden at the second key"
    );
    let eased = |kind: &str| {
        evaluate(&format!(
            "b = solid.box@1(size=[1, 1, 1])\n\
             t = anim.track@1(b, property=\"position\", keys=\"0:0,0,0; 1:1,0,0\", interpolation=\"{kind}\", loop=false)\noutput o = t\n"
        ))
        .unwrap()
        .parts[0]
            .animation[0]
            .sample(0.5)[0]
    };
    assert!(
        (eased("ease_in") - 0.25).abs() < 1e-6,
        "ease_in is slow at first"
    );
    assert!(
        (eased("ease_out") - 0.75).abs() < 1e-6,
        "ease_out is fast at first"
    );
}

#[test]
fn flag_logic_is_validated_when_the_behaviour_is_built() {
    let part = |params: &str| {
        evaluate(&format!(
            "b = solid.box@1(size=[1, 1, 1])\nt = anim.track@1(b, property=\"position\", keys=\"0:0,0,0; 1:0,1,0\")\n\
             r = behavior.on@1(t{params})\noutput o = r\n"
        ))
    };
    let ok = part(", trigger=\"select\", needs=\"key; !alarm\", sets=\"door=toggle\"")
        .unwrap()
        .parts
        .remove(0);
    let behavior = ok.behavior.unwrap();
    assert_eq!(behavior.needs.len(), 2);
    assert!(!behavior.needs[1].value, "!alarm needs the alarm off");
    assert_eq!(behavior.sets[0].name, "door");
    let watcher = part(", trigger=\"flag\", watch=\"powered\", action=\"follow\"")
        .unwrap()
        .parts
        .remove(0);
    assert_eq!(watcher.behavior.unwrap().watch.as_deref(), Some("powered"));
    for (bad, why) in [
        (", trigger=\"flag\"", "watch"),
        (", trigger=\"select\", watch=\"x\"", "watch"),
        (", trigger=\"flag\", watch=\"x\", sets=\"x\"", "itself"),
        (", trigger=\"select\", action=\"follow\"", "follow"),
        (", sets=\"Bad Name\"", "flag name"),
        (", needs=\"a=maybe\"", "on or off"),
    ] {
        let error = part(bad)
            .err()
            .unwrap_or_else(|| panic!("{bad} should be refused"));
        assert!(error.1.contains(why), "{bad}: {error:?}");
    }
    let logic_only = "g = solid.box@1(size=[1, 1, 1])\nr = behavior.on@1(g, trigger=\"flag\", watch=\"a\", sets=\"b\")\noutput o = r\n";
    assert!(
        evaluate(logic_only).is_ok(),
        "a part that only sets flags needs no clip"
    );
}

#[test]
fn colour_tracks_fade_between_colours_and_stay_in_range() {
    let colour = |keys: &str| {
        evaluate(&format!(
            "b = solid.box@1(size=[1, 1, 1])\nm = solid.material@1(b, color=[1, 0, 0])\n\
             c = anim.track@1(m, property=\"color\", keys=\"{keys}\", interpolation=\"linear\", loop=false)\noutput o = c\n"
        ))
    };
    let part = colour("0:1,0,0; 1:0,0,1").unwrap().parts.remove(0);
    assert_eq!(part.animation[0].property, TrackProperty::Color);
    let middle = part.animation[0].sample(0.5);
    assert!(
        (middle[0] - 0.5).abs() < 1e-6 && (middle[2] - 0.5).abs() < 1e-6,
        "halfway between red and blue"
    );
    assert!(
        colour("0:1,0,0; 1:2,0,0").is_err(),
        "a channel above 1 is refused"
    );
    assert!(
        colour("0:1,0,0; 1:-0.1,0,0").is_err(),
        "a negative channel is refused"
    );
    assert!(
        colour("0:1; 1:0").is_err(),
        "a colour key needs all three channels"
    );
}

#[test]
fn every_interpolation_option_builds_a_track_that_reaches_its_last_key() {
    for kind in [
        "linear", "smooth", "ease_in", "ease_out", "bezier", "back", "bounce", "elastic", "step",
    ] {
        let part = &evaluate(&format!(
            "b = solid.box@1(size=[1, 1, 1])\n\
             t = anim.track@1(b, property=\"position\", keys=\"0:0,0,0; 1:1,2,0; 3:4,0,1\", interpolation=\"{kind}\", loop=false)\noutput o = t\n"
        ))
        .unwrap()
        .parts[0];
        let track = &part.animation[0];
        assert_eq!(
            track.sample(3.0),
            [4.0, 0.0, 1.0],
            "{kind} ends on the last key"
        );
        assert_eq!(
            track.sample(1.0),
            [1.0, 2.0, 0.0],
            "{kind} passes through the middle key"
        );
        assert!(
            track.sample(0.5).iter().all(|v| v.is_finite()),
            "{kind} is finite between keys"
        );
    }
}

#[test]
fn bezier_flows_through_a_middle_key_where_smooth_stops_at_it() {
    let at = |kind: &str, time: f32| {
        evaluate(&format!(
            "b = solid.box@1(size=[1, 1, 1])\n\
             t = anim.track@1(b, property=\"position\", keys=\"0:0,0,0; 1:1,0,0; 2:2,0,0\", interpolation=\"{kind}\", loop=false)\noutput o = t\n"
        ))
        .unwrap()
        .parts[0]
            .animation[0]
            .sample(time)[0]
    };
    // Around the middle key of an even, straight run: smooth slows to zero speed there, bezier does not.
    let speed = |kind: &str| (at(kind, 1.05) - at(kind, 0.95)) / 0.1;
    assert!(
        speed("smooth") < 0.2,
        "smooth stops at each key: {}",
        speed("smooth")
    );
    assert!(
        speed("bezier") > 0.8,
        "bezier keeps moving through it: {}",
        speed("bezier")
    );
}

const TWO_CLIPS: &str = "arm = solid.box@1(size=[0.1, 0.6, 0.1])\n\
    wave = anim.track@1(arm, property=\"rotation\", keys=\"0:0,0,0; 1:0,0,40\", clip=\"wave\", loop=false)\n\
    reach = anim.track@1(wave, property=\"rotation\", keys=\"0:0,0,0; 2:60,0,0\", clip=\"reach\", loop=false)\n";

#[test]
fn clips_group_tracks_and_behaviours_pick_them_by_name() {
    let part = |tail: &str| {
        evaluate(&format!("{TWO_CLIPS}{tail}")).map(|e| e.parts.into_iter().next().unwrap())
    };
    let ok = part("r = behavior.on@1(reach, trigger=\"select\", clip=\"reach\", then=\"wave\", action=\"play\")\noutput o = r\n").unwrap();
    let clips: Vec<&str> = ok.animation.iter().map(|t| t.clip.as_str()).collect();
    assert_eq!(clips, ["wave", "reach"]);
    let behavior = ok.behavior.unwrap();
    assert_eq!(
        (behavior.clip.as_str(), behavior.then.as_slice()),
        ("reach", &["wave".to_owned()][..])
    );
    for (bad, why) in [
        (
            "r = behavior.on@1(reach, trigger=\"select\", clip=\"nope\")\noutput o = r\n",
            "no clip named",
        ),
        (
            "r = behavior.on@1(reach, trigger=\"select\", clip=\"reach\", then=\"reach\")\noutput o = r\n",
            "then",
        ),
        (
            "r = behavior.on@1(reach, trigger=\"select\", clip=\"wave\", then=\"ghost\")\noutput o = r\n",
            "then",
        ),
        (
            "r = behavior.on@1(reach, trigger=\"select\", clip=\"Bad Name\")\noutput o = r\n",
            "clip name",
        ),
    ] {
        let error = part(bad)
            .err()
            .unwrap_or_else(|| panic!("{bad} should be refused"));
        assert!(error.1.contains(why), "{bad}: {error:?}");
    }
    let sender = format!(
        "{TWO_CLIPS}listen = behavior.on@1(reach, trigger=\"signal\")\noutput arm_out = listen\n\
         b = solid.box@1(size=[0.1, 0.1, 0.1])\nbutton = behavior.on@1(b, trigger=\"select\", target=\"arm_out:wave\")\noutput button_out = button\n"
    );
    let parts = evaluate(&sender).unwrap().parts;
    let button = parts
        .iter()
        .find(|p| p.name == "button_out")
        .unwrap()
        .behavior
        .clone()
        .unwrap();
    assert_eq!(
        (button.target.as_deref(), button.target_clip.as_str()),
        (Some("arm_out"), "wave")
    );
    assert!(evaluate(&format!("{TWO_CLIPS}x = anim.track@1(reach, property=\"position\", keys=\"0:0,0,0; 1:1,0,0\", clip=\"Bad\")\noutput o = x\n")).is_err());
}

#[test]
fn a_placed_set_carries_its_tracks_with_a_phase_per_copy() {
    let field = "reed = solid.box@1(size=[0.05, 0.6, 0.05])\nrow = inst.array@1(reed, count_x=5, spacing=[0.3, 0, 0])\n\
        sway = anim.wave@1(row, property=\"rotation\", amplitude=[0, 0, 10], period=2, pivot=[0, -0.3, 0], phase_step=0.2, phase_jitter=0.5)\n\
        output field_out = sway\n";
    let part = &evaluate(field).unwrap().parts[0];
    assert_eq!(
        part.transforms.as_ref().map(Vec::len),
        Some(5),
        "five placed copies"
    );
    let [track] = &part.animation[..] else {
        panic!("the set carries its track: {:?}", part.animation)
    };
    assert_eq!((track.phase_step, track.phase_jitter), (0.2, 0.5));
    assert!(
        track.phase(3) > track.phase(2) - 0.5,
        "later copies run ahead on average"
    );
    let single = "b = solid.box@1(size=[1, 1, 1])\nw = anim.wave@1(b, property=\"position\", amplitude=[0, 1, 0], phase_step=0.2)\noutput o = w\n";
    assert!(
        evaluate(single).err().unwrap().1.contains("placed set"),
        "phase means nothing for a single part"
    );
    let plain = "reed = solid.box@1(size=[0.05, 0.6, 0.05])\nrow = inst.array@1(reed, count_x=3, spacing=[0.3, 0, 0])\n\
        move = anim.track@1(row, property=\"position\", keys=\"0:0,0,0; 1:0,1,0\")\noutput o = move\n";
    assert!(
        evaluate(plain).is_ok(),
        "a set may move all together with no phase at all"
    );
}
