//! Renders every sound effect and one piece of music per mood into a folder as WAV files, so they can be listened to.
//!
//!   cargo run -p ve-audio --example showcase -- <folder>
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "an example reports failures by stopping"
)]
use ve_audio::{ALL_SFX, Mood, MusicSpec, Scale, music, sfx, to_wav_bytes};

fn main() {
    let folder = std::env::args().nth(1).expect("give an output folder");
    std::fs::create_dir_all(&folder).unwrap();
    for kind in ALL_SFX {
        let sound = sfx(kind, 7, 0.8);
        let name = format!("{kind:?}").to_lowercase();
        std::fs::write(format!("{folder}/sfx_{name}.wav"), to_wav_bytes(&sound)).unwrap();
    }
    let moods = [
        (Mood::Calm, Scale::Pentatonic, 72.0),
        (Mood::Tense, Scale::Minor, 110.0),
        (Mood::Heroic, Scale::Major, 124.0),
        (Mood::Mysterious, Scale::Minor, 84.0),
        (Mood::Playful, Scale::Major, 138.0),
    ];
    for (mood, scale, tempo_bpm) in moods {
        let sound = music(MusicSpec {
            seed: 11,
            tempo_bpm,
            bars: 8,
            scale,
            mood,
        });
        let name = format!("{mood:?}").to_lowercase();
        std::fs::write(format!("{folder}/music_{name}.wav"), to_wav_bytes(&sound)).unwrap();
    }
}
