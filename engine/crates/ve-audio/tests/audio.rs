//! Integration tests for procedural audio behavior.
#[cfg(test)]
mod tests {
    use ve_audio::*;
    const PI2: f32 = core::f32::consts::TAU;
    fn sin(x: f32) -> f32 {
        x.sin()
    }

    #[test]
    fn seeded_effects_are_bounded_and_complete() {
        for kind in ALL_SFX {
            let a = sfx(kind, 12, 0.8);
            let b = sfx(kind, 12, 0.8);
            let c = sfx(kind, 13, 0.8);
            assert_eq!(a, b);
            assert_ne!(a.samples, c.samples);
            assert!(a.samples.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
            assert!(!a.samples.is_empty());
        }
        assert!(
            sfx(SfxKind::Coin, 0, 1.0).samples.len() < sfx(SfxKind::RainLoop, 0, 1.0).samples.len()
        );
    }
    #[test]
    fn sine_is_at_440_hz_and_lowpass_reduces_high_tone() {
        let patch = Patch {
            amplitude: Envelope {
                attack: 0.0,
                decay: 0.0,
                sustain: 1.0,
                release: 0.0,
            },
            ..Patch::default()
        };
        let sound = render(&patch, 440.0, 1.0, 1);
        let energy = |freq: f32| {
            let mut re = 0.0;
            let mut im = 0.0;
            for (i, x) in sound.samples.iter().enumerate() {
                let angle = PI2 * freq * i as f32 / sound.sample_rate as f32;
                re += *x * sin(angle + 1.5707963);
                im += *x * sin(angle);
            }
            re * re + im * im
        };
        assert!(energy(440.0) > energy(430.0) * 50.0);
        let mut mixed = Patch {
            oscillators: vec![
                Oscillator {
                    waveform: Waveform::Sine,
                    detune_semitones: 0.0,
                    mix: 0.5,
                },
                Oscillator {
                    waveform: Waveform::Sine,
                    detune_semitones: 12.0,
                    mix: 0.5,
                },
            ],
            ..patch.clone()
        };
        let raw = render(&mixed, 440.0, 0.2, 1);
        mixed.filter = Some(Filter {
            kind: FilterKind::LowPass,
            cutoff_hz: 600.0,
        });
        let filtered = render(&mixed, 440.0, 0.2, 1);
        let high_energy = |s: &Sound| {
            let mut re = 0.0;
            let mut im = 0.0;
            for (i, x) in s.samples.iter().enumerate() {
                let a = PI2 * 880.0 * i as f32 / s.sample_rate as f32;
                re += *x * sin(a + 1.5707963);
                im += *x * sin(a);
            }
            re * re + im * im
        };
        assert!(high_energy(&filtered) < high_energy(&raw));
    }
    #[test]
    fn music_loops_and_wav_has_consistent_header() {
        let spec = MusicSpec {
            seed: 3,
            tempo_bpm: 120.0,
            bars: 2,
            scale: Scale::Major,
            mood: Mood::Calm,
        };
        let a = music(spec);
        let b = music(spec);
        assert_eq!(a, b);
        assert_eq!(a.samples.len(), 192_000);
        assert!((a.samples[0] - a.samples[a.samples.len() - 1]).abs() < 0.01);
        let wav = to_wav_bytes(&a);
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(
            u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]) as usize,
            a.samples.len() * 2
        );
    }
    #[test]
    fn spatial_gain_pan_and_doppler_are_sensible() {
        let origin = Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let forward = Vec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        };
        let left = spatialise(
            origin,
            forward,
            Vec3 {
                x: -1.0,
                y: 0.0,
                z: -1.0,
            },
            1.0,
            1.0,
        );
        let right = spatialise(
            origin,
            forward,
            Vec3 {
                x: 1.0,
                y: 0.0,
                z: -1.0,
            },
            1.0,
            1.0,
        );
        let far = spatialise(
            origin,
            forward,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: -10.0,
            },
            1.0,
            1.0,
        );
        assert!(left.pan < 0.0 && right.pan > 0.0);
        assert!(far.gain < left.gain);
        let centre = spatialise(
            origin,
            forward,
            Vec3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
            1.0,
            1.0,
        );
        assert!((centre.gain * 0.70710677 - 0.70710677).abs() < 0.001);
        assert!((doppler_ratio(0.0, 0.0, 343.0) - 1.0).abs() < 0.0001);
    }
}
