
/// Reads the `trigger` and `action` names.
fn kinds(trigger: &str, action: &str) -> Result<(Trigger, Action), Fail> {
    let trigger = match trigger {
        "select" => Trigger::Select,
        "near" => Trigger::Near,
        "signal" => Trigger::Signal,
        "flag" => Trigger::Flag,
        "grab" => Trigger::Grab,
        "release" => Trigger::Release,
        "hold" => Trigger::Hold,
        "timer" => Trigger::Timer,
        "socket" => Trigger::Socket,
        "collide" => Trigger::Collide,
        other => {
            return Err(fail(
                "param.range",
                format!("unknown behavior.on trigger \"{other}\""),
            ));
        }
    };
    let action = match action {
        "play" => Action::Play,
        "toggle" => Action::Toggle,
        "follow" => Action::Follow,
        other => {
            return Err(fail(
                "param.range",
                format!("unknown behavior.on action \"{other}\""),
            ));
        }
    };
    Ok((trigger, action))
}

/// Reads and checks the timer's `after` and `every` seconds; they only go with a timer trigger.
fn timer(node: &Node, trigger: Trigger) -> Result<(f32, f32), Fail> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "timer seconds are bounded by the spec"
    )]
    let (after, every) = (
        node.params
            .get("after")
            .and_then(Param::as_f64)
            .unwrap_or(0.0) as f32,
        node.params
            .get("every")
            .and_then(Param::as_f64)
            .unwrap_or(0.0) as f32,
    );
    if !(0.0..=MAX_TIMER).contains(&after)
        || !(every == 0.0 || (MIN_EVERY..=MAX_TIMER).contains(&every))
    {
        return Err(fail(
            "behavior.timer",
            format!(
                "timer `after` must be 0 to {MAX_TIMER} seconds and `every` 0 (once) or {MIN_EVERY} to {MAX_TIMER} seconds"
            ),
        ));
    }
    if trigger != Trigger::Timer && (after > 0.0 || every > 0.0) {
        return Err(fail(
            "behavior.timer",
            "`after` and `every` only go with trigger=\"timer\"",
        ));
    }
    Ok((after, every))
}

/// Attaches the node's behaviour to the input; it needs a track to run and allows only one behaviour.
pub(crate) fn behavior(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "behavior.on needs a shape"))?;
    let (Some(Param::Str(trigger)), Some(Param::Str(action))) =
        (node.params.get("trigger"), node.params.get("action"))
    else {
        return Err(fail("param.type", "behavior.on needs trigger and action"));
    };
    let (trigger, action) = kinds(trigger, action)?;
    let text = |key: &str| match node.params.get(key) {
        Some(Param::Str(text)) => text.as_str(),
        _ => "",
    };
    let sets = flags::parse_sets(text("sets"))?;
    let needs = flags::parse_needs(text("needs"))?;
    let watch = match text("watch").trim() {
        "" => None,
        name => Some(flags::name(name)?),
    };
    if (trigger == Trigger::Flag) != watch.is_some() {
        return Err(fail(
            "behavior.watch",
            "trigger=\"flag\" needs a watch flag, and watch only goes with trigger=\"flag\"",
        ));
    }
    if let Some(watched) = &watch
        && sets.iter().any(|set| &set.name == watched)
    {
        return Err(fail(
            "behavior.watch",
            format!(
                "a behaviour that watches \"{watched}\" cannot also set it (that would trigger itself forever)"
            ),
        ));
    }
    if action == Action::Follow && trigger != Trigger::Flag {
        return Err(fail(
            "behavior.follow",
            "action=\"follow\" only works with trigger=\"flag\"",
        ));
    }
    #[expect(clippy::cast_possible_truncation, reason = "radius is bounded metres")]
    let radius = node
        .params
        .get("radius")
        .and_then(Param::as_f64)
        .unwrap_or(2.0) as f32;
    if !(RADIUS_RANGE.0..=RADIUS_RANGE.1).contains(&radius) {
        return Err(fail(
            "param.range",
            "behavior.on radius must be 0.1 to 100 metres",
        ));
    }
    let (target, target_clip) = match node.params.get("target") {
        Some(Param::Str(text)) if !text.trim().is_empty() => {
            let (name, clip) = text.trim().split_once(':').unwrap_or((text.trim(), ""));
            (Some(name.trim().to_owned()), clip_text(clip)?)
        }
        _ => (None, String::new()),
    };
    if trigger == Trigger::Signal && target.is_some() {
        return Err(fail(
            "behavior.signal_target",
            "a signal behaviour only listens; drop its target (put the target on the part that sends)",
        ));
    }
    if target.is_none() && sets.is_empty() && input.animation.is_empty() {
        return Err(fail(
            "behavior.no_track",
            "behavior.on needs an anim.track upstream to run, a target part to run, or flags to set; add anim.track before it, or set target or sets",
        ));
    }
    if input.behavior.is_some() {
        return Err(fail(
            "behavior.duplicate",
            "a part can carry only one behavior.on",
        ));
    }
    let (after, every) = timer(node, trigger)?;
    let mut value = (**input).clone();
    let (clip, then) = own_clips(node, input, trigger)?;
    let accepts = accepted_part(node, trigger)?;
    value.behavior = Some(Behavior {
        trigger,
        target,
        radius,
        action,
        sets,
        needs,
        watch,
        after,
        every,
        accepts,
        clip,
        target_clip,
        then,
    });
    Ok(value)
}
