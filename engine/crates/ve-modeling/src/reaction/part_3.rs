
#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-6
    }

    fn reaction(action: Action) -> Reaction {
        Reaction::new(
            Behavior {
                trigger: Trigger::Select,
                target: None,
                radius: 2.0,
                action,
                sets: Vec::new(),
                needs: Vec::new(),
                watch: None,
                after: 0.0,
                every: 0.0,
                accepts: None,
                clip: String::new(),
                target_clip: String::new(),
                then: Vec::new(),
            },
            0.5,
        )
    }

    #[test]
    fn toggle_runs_forward_then_back_and_stops_at_each_end() {
        let mut door = reaction(Action::Toggle);
        door.step();
        assert!(near(door.time, 0.0), "nothing moves before a trigger");
        door.trigger();
        for _ in 0..40 {
            door.step();
        }
        assert!(!door.running && near(door.time, 0.5), "held open: {door:?}");
        door.trigger();
        for _ in 0..40 {
            door.step();
        }
        assert!(
            !door.running && near(door.time, 0.0),
            "closed again: {door:?}"
        );
        door.trigger();
        for _ in 0..10 {
            door.step();
        }
        door.trigger();
        let midway = door.time;
        door.step();
        assert!(
            door.time < midway,
            "a trigger mid-swing reverses from where it is"
        );
    }

    #[test]
    fn a_timer_fires_once_after_its_delay_and_then_every_interval() {
        let fired = |after, every, steps: u64| {
            (0..steps)
                .filter(|s| timer_due(after, every, *s))
                .collect::<Vec<_>>()
        };
        assert_eq!(fired(2.0, 0.0, 400), [120], "once, 120 steps (2 s) in");
        assert_eq!(fired(0.0, 0.0, 100), [0], "after 0 fires on the first step");
        assert_eq!(
            fired(1.0, 0.5, 200),
            [60, 90, 120, 150, 180],
            "then every half second"
        );
        assert_eq!(
            fired(1.0, 0.5, 60),
            Vec::<u64>::new(),
            "nothing before the delay"
        );
        assert_eq!(
            fired(0.0, 0.1, 20),
            [0, 6, 12, 18],
            "the shortest repeat is six steps"
        );
    }

    #[test]
    fn follow_heads_for_the_end_that_matches_the_flag() {
        let mut lamp = reaction(Action::Follow);
        lamp.follow(false);
        assert!(
            !lamp.running && near(lamp.time, 0.0),
            "already at the off end: nothing to do"
        );
        lamp.follow(true);
        for _ in 0..10 {
            lamp.step();
        }
        lamp.follow(false);
        let midway = lamp.time;
        lamp.step();
        assert!(
            lamp.time < midway,
            "turning off mid-way reverses from where it is"
        );
        for _ in 0..40 {
            lamp.step();
        }
        assert!(!lamp.running && near(lamp.time, 0.0));
        lamp.follow(true);
        lamp.follow(true);
        for _ in 0..40 {
            lamp.step();
        }
        assert!(
            !lamp.running && near(lamp.time, 0.5),
            "repeating the same value changes nothing"
        );
    }

    #[test]
    fn play_runs_once_and_ignores_triggers_while_running() {
        let mut lever = reaction(Action::Play);
        lever.trigger();
        for _ in 0..10 {
            lever.step();
        }
        let before = lever.time;
        lever.trigger();
        assert!(near(lever.time, before), "ignored while running");
        for _ in 0..40 {
            lever.step();
        }
        assert!(!lever.running && near(lever.time, 0.5));
        lever.trigger();
        assert!(near(lever.time, 0.0), "starts over once it has finished");
        let mut again = reaction(Action::Play);
        again.trigger();
        for _ in 0..20 {
            again.step();
        }
        let mut other = reaction(Action::Play);
        other.trigger();
        for _ in 0..20 {
            other.step();
        }
        assert_eq!(again, other, "same triggers, same step numbers, same state");
    }
}
