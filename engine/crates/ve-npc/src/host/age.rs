//! Human-readable elapsed times for prompt memories.

pub(super) fn format_age(seconds: f64) -> String {
    let seconds = seconds.max(0.0);
    if seconds < 5.0 {
        "just now".into()
    } else if seconds < 60.0 {
        quantity(seconds, "second")
    } else if seconds < 3_600.0 {
        quantity(seconds / 60.0, "minute")
    } else if seconds < 86_400.0 {
        quantity(seconds / 3_600.0, "hour")
    } else {
        quantity(seconds / 86_400.0, "day")
    }
}

fn quantity(seconds: f64, unit: &str) -> String {
    let amount = seconds.floor();
    let suffix = if (1.0..2.0).contains(&amount) {
        ""
    } else {
        "s"
    };
    format!("{amount:.0} {unit}{suffix} ago")
}
