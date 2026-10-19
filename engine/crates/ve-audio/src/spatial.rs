use super::Sound;

/// 3D point used by spatial audio functions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
}
/// Spatial attenuation and equal-power pan result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spatial {
    /// Linear gain.
    pub gain: f32,
    /// Pan from -1 left to +1 right.
    pub pan: f32,
}
/// Inverse-distance attenuation and equal-power pan for a source.
#[must_use]
pub fn spatialise(
    listener_pos: Vec3,
    listener_forward: Vec3,
    source_pos: Vec3,
    ref_dist: f32,
    rolloff: f32,
) -> Spatial {
    let dx = source_pos.x - listener_pos.x;
    let dy = source_pos.y - listener_pos.y;
    let dz = source_pos.z - listener_pos.z;
    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
    let gain = ref_dist.max(0.001)
        / (ref_dist.max(0.001) + rolloff.max(0.0) * (dist - ref_dist.max(0.001)).max(0.0));
    let right_x = -listener_forward.z;
    let right_z = listener_forward.x;
    let side = dx * right_x + dz * right_z;
    let denom = (dx * dx + dz * dz).sqrt().max(0.0001);
    let pan = (side / denom).clamp(-1.0, 1.0);
    Spatial { gain, pan }
}
/// Doppler pitch multiplier using source and listener radial velocities in metres per second.
#[must_use]
pub fn doppler_ratio(
    listener_radial_velocity: f32,
    source_radial_velocity: f32,
    speed_of_sound: f32,
) -> f32 {
    let c = speed_of_sound.max(1.0);
    ((c + listener_radial_velocity) / (c + source_radial_velocity)).clamp(0.25, 4.0)
}

/// Encode mono PCM as a little-endian 16-bit RIFF/WAVE file.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    reason = "sample data length is clamped to the RIFF u32 limit before conversion"
)]
pub fn to_wav_bytes(sound: &Sound) -> Vec<u8> {
    let data_len = u32::try_from(sound.samples.len().saturating_mul(2).min(u32::MAX as usize))
        .unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36_u32.saturating_add(data_len)).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&sound.sample_rate.to_le_bytes());
    out.extend_from_slice(&sound.sample_rate.saturating_mul(2).to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in sound.samples.iter().take(data_len as usize / 2) {
        let v = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}
