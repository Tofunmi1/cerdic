//! A CPU reference for the physically based atmosphere (the Bruneton / Hillaire model Bevy uses): Rayleigh, Mie and ozone, on a
//! spherical planet. It exists so the GPU lookup-table passes can be checked against numbers computed in `f64`.
//!
//! Units are metres and per-metre coefficients. Colours are linear RGB. Directions are in a frame whose +y is straight up at the
//! point being looked from.

/// The planet and its air.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Atmosphere {
    /// Radius of the ground sphere, in metres.
    pub bottom_radius: f64,
    /// Radius of the top of the atmosphere, in metres.
    pub top_radius: f64,
    /// Rayleigh scattering coefficient per metre at sea level, per colour channel.
    pub rayleigh_scattering: [f64; 3],
    /// Height over which Rayleigh density falls by a factor of e, in metres.
    pub rayleigh_scale_height: f64,
    /// Mie scattering coefficient per metre at sea level.
    pub mie_scattering: f64,
    /// Mie absorption coefficient per metre at sea level.
    pub mie_absorption: f64,
    /// Height over which Mie density falls by a factor of e, in metres.
    pub mie_scale_height: f64,
    /// Mie phase asymmetry (Henyey-Greenstein g).
    pub mie_asymmetry: f64,
    /// Ozone absorption coefficient per metre at the layer's peak density, per colour channel.
    pub ozone_absorption: [f64; 3],
}

impl Atmosphere {
    /// The Earth-like atmosphere Bevy and Hillaire use: ground 6360 km, top 6460 km, Rayleigh `[5.802e-6, 13.558e-6, 33.1e-6]` with
    /// scale height 8 km, Mie scattering 3.996e-6 and absorption 4.44e-6 with scale height 1.2 km and asymmetry 0.8, ozone absorption
    /// `[0.65e-6, 1.881e-6, 0.085e-6]` in a tent-shaped layer that is zero at 10 km, one at 25 km and zero at 40 km.
    #[must_use]
    pub fn earth() -> Self {
        Self {
            bottom_radius: 6_360_000.0,
            top_radius: 6_460_000.0,
            rayleigh_scattering: [5.802e-6, 13.558e-6, 33.1e-6],
            rayleigh_scale_height: 8_000.0,
            mie_scattering: 3.996e-6,
            mie_absorption: 4.44e-6,
            mie_scale_height: 1_200.0,
            mie_asymmetry: 0.8,
            ozone_absorption: [0.65e-6, 1.881e-6, 0.085e-6],
        }
    }

    /// Extinction (scattering plus absorption) per metre, per channel, at `height` metres above the ground.
    #[must_use]
    pub fn extinction(&self, height: f64) -> [f64; 3] {
        let rayleigh_density = (-height / self.rayleigh_scale_height).exp();
        let mie_density = (-height / self.mie_scale_height).exp();
        let ozone_density = (1.0 - (height - 25_000.0).abs() / 15_000.0).max(0.0);
        let mie_extinction = (self.mie_scattering + self.mie_absorption) * mie_density;
        std::array::from_fn(|channel| {
            self.rayleigh_scattering[channel] * rayleigh_density
                + mie_extinction
                + self.ozone_absorption[channel] * ozone_density
        })
    }

    /// Distance along a ray from `radius` (distance from the planet's centre) with cosine `mu` against the local up direction to
    /// where it leaves the top of the atmosphere.
    #[must_use]
    pub fn distance_to_top(&self, radius: f64, mu: f64) -> f64 {
        let discriminant = radius * radius * (mu * mu - 1.0) + self.top_radius * self.top_radius;
        -radius * mu + discriminant.max(0.0).sqrt()
    }

    /// Distance to the ground sphere, or `None` when the ray does not hit it (it points up, or passes beside the planet).
    #[must_use]
    pub fn distance_to_ground(&self, radius: f64, mu: f64) -> Option<f64> {
        if mu >= 0.0 {
            return None;
        }
        let discriminant =
            radius * radius * (mu * mu - 1.0) + self.bottom_radius * self.bottom_radius;
        if discriminant < 0.0 {
            return None;
        }
        let distance = -radius * mu - discriminant.sqrt();
        (distance >= 0.0).then_some(distance)
    }

    /// The integral of extinction along the first `distance` metres of the ray, following the planet's curvature, per channel.
    #[must_use]
    pub fn optical_depth(&self, radius: f64, mu: f64, distance: f64) -> [f64; 3] {
        if distance <= 0.0 {
            return [0.0; 3];
        }
        let steps = (distance / 250.0).ceil().max(256.0);
        let ds = distance / steps;
        let mut depth = [0.0; 3];
        let mut step = 0.0;
        while step < steps {
            let s = (step + 0.5) * ds;
            let sample_radius = (radius * radius + 2.0 * radius * mu * s + s * s).sqrt();
            let extinction = self.extinction(sample_radius - self.bottom_radius);
            for channel in 0..3 {
                depth[channel] += extinction[channel] * ds;
            }
            step += 1.0;
        }
        depth
    }

    /// The share of light that survives from the point out through the top of the atmosphere along the ray, per channel.
    #[must_use]
    pub fn transmittance_to_top(&self, radius: f64, mu: f64) -> [f64; 3] {
        let distance = self.distance_to_top(radius, mu);
        self.transmittance(radius, mu, distance)
    }

    /// The share of light that survives along the first `distance` metres of the ray, per channel.
    #[must_use]
    pub fn transmittance(&self, radius: f64, mu: f64, distance: f64) -> [f64; 3] {
        self.optical_depth(radius, mu, distance)
            .map(|depth| (-depth).exp())
    }

    /// Single-scattered sky radiance reaching a viewer at `radius` from the planet's centre, looking along `view` (a unit vector in the
    /// local frame, +y up), with the sun in direction `sun` (unit, same frame) and `sun_illuminance` the sunlight arriving at the top
    /// of the atmosphere, per channel. The view ray runs to the top of the atmosphere, or to the ground if it hits it. Ground
    /// reflection and multiple scattering are left out. Rayleigh uses `3/(16 pi) (1 + cos^2)`, Mie the Henyey-Greenstein phase.
    #[must_use]
    pub fn single_scattering(
        &self,
        radius: f64,
        view: [f64; 3],
        sun: [f64; 3],
        sun_illuminance: [f64; 3],
    ) -> [f64; 3] {
        const VIEW_STEPS: f64 = 128.0;
        let view_mu = view[1];
        let end = self
            .distance_to_ground(radius, view_mu)
            .unwrap_or_else(|| self.distance_to_top(radius, view_mu));
        let ds = end / VIEW_STEPS;
        let cosine = (view[0] * sun[0] + view[1] * sun[1] + view[2] * sun[2]).clamp(-1.0, 1.0);
        let rayleigh_phase = 3.0 / (16.0 * std::f64::consts::PI) * (1.0 + cosine * cosine);
        let g = self.mie_asymmetry;
        let mie_phase = (1.0 - g * g)
            / (4.0 * std::f64::consts::PI * (1.0 + g * g - 2.0 * g * cosine).powf(1.5));
        let mut radiance = [0.0; 3];
        let mut step = 0.0;
        while step < VIEW_STEPS {
            let s = (step + 0.5) * ds;
            let px = s * view[0];
            let py = radius + s * view[1];
            let pz = s * view[2];
            let sample_radius = (px * px + py * py + pz * pz).sqrt();
            let sample_up = [px / sample_radius, py / sample_radius, pz / sample_radius];
            let sun_mu = sample_up[0] * sun[0] + sample_up[1] * sun[1] + sample_up[2] * sun[2];
            if self.distance_to_ground(sample_radius, sun_mu).is_some() {
                step += 1.0;
                continue;
            }
            let view_transmittance = self.transmittance(radius, view_mu, s);
            let sun_transmittance = self.transmittance_to_top(sample_radius, sun_mu);
            let height = sample_radius - self.bottom_radius;
            let rayleigh_density = (-height / self.rayleigh_scale_height).exp();
            let mie_density = (-height / self.mie_scale_height).exp();
            for channel in 0..3 {
                let scattering =
                    self.rayleigh_scattering[channel] * rayleigh_density * rayleigh_phase
                        + self.mie_scattering * mie_density * mie_phase;
                radiance[channel] += sun_illuminance[channel]
                    * view_transmittance[channel]
                    * sun_transmittance[channel]
                    * scattering
                    * ds;
            }
            step += 1.0;
        }
        radiance
    }
}
