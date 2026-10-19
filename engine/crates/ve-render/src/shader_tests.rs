//! Every shader parses and validates. A WGSL mistake otherwise shows up only as a silently missing pass in a browser (the
//! lens, the headset split or every mesh stops drawing), so it is caught here.
use naga::valid::{Capabilities, ValidationFlags, Validator};

fn check(name: &str, source: &str) {
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{name} does not parse:\n{}", error.emit_to_string(source)));
    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .unwrap_or_else(|error| panic!("{name} does not validate: {error:?}"));
}

#[test]
fn the_forward_shader_parses_and_validates() {
    check("forward.wgsl", include_str!("shaders/forward.wgsl"));
}

#[test]
fn the_present_shader_parses_and_validates() {
    check("present.wgsl", include_str!("shaders/present.wgsl"));
}

#[test]
fn the_shadow_shader_parses_and_validates() {
    check("shadow.wgsl", include_str!("shaders/shadow.wgsl"));
}

#[test]
fn the_sky_shader_parses_and_validates() {
    check("sky.wgsl", include_str!("shaders/sky.wgsl"));
}
