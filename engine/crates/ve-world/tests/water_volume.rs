//! The small CPU 3D liquid reference kernel.
use ve_world::water_sim::volume::{Error, Options, Particle, Volume};

fn options(gravity: [f32; 3]) -> Options {
    Options {
        min: [-1.0; 3],
        max: [1.0; 3],
        smoothing_radius: 0.5,
        rest_density: 1.0,
        particle_mass: 1.0,
        gravity,
        damping: 0.0,
        iterations: 3,
    }
}

#[test]
fn particles_move_in_all_three_axes() -> Result<(), Error> {
    let particle = Particle {
        position: [0.0; 3],
        velocity: [0.0; 3],
    };
    let mut volume = Volume::new(vec![particle], options([1.0, -2.0, 3.0]))?;
    volume.step(0.01)?;
    let p = volume.particles()[0].position;
    assert!(p[0] > 0.0 && p[1] < 0.0 && p[2] > 0.0);
    assert!((volume.time() - 0.01).abs() < 1e-8);
    Ok(())
}

#[test]
fn box_fill_occupies_volume_and_obeys_budget() -> Result<(), Error> {
    let mut o = options([0.0; 3]);
    o.min = [0.0; 3];
    o.max = [0.3; 3];
    let volume = Volume::filled_box(o, 0.1)?;
    assert_eq!(volume.particles().len(), 27);
    for axis in 0..3 {
        assert!(
            volume
                .particles()
                .iter()
                .any(|p| (p.position[axis] - volume.particles()[0].position[axis]).abs() > 1e-6)
        );
    }
    Ok(())
}

#[test]
fn density_projection_separates_compressed_particles() -> Result<(), Error> {
    let mut o = options([0.0; 3]);
    // A particle at the kernel centre alone has exactly this density; a nearby second particle
    // compresses the pair and the PBF constraints should separate it.
    o.rest_density = 315.0 / (64.0 * std::f32::consts::PI * o.smoothing_radius.powi(3));
    let particles = vec![
        Particle {
            position: [0.0, -0.1, 0.0],
            velocity: [0.0; 3],
        },
        Particle {
            position: [0.0, 0.1, 0.0],
            velocity: [0.0; 3],
        },
    ];
    let mut volume = Volume::new(particles, o)?;
    volume.step(0.01)?;
    let separation = volume.particles()[0].position[1] - volume.particles()[1].position[1];
    assert!(
        separation < -0.1,
        "density constraint should push particles apart: {separation}"
    );
    Ok(())
}

#[test]
fn rejects_invalid_inputs_and_steps() -> Result<(), Error> {
    assert_eq!(
        Volume::new(Vec::new(), options([0.0; 3])).err(),
        Some(Error::Empty)
    );
    assert_eq!(
        Volume::new(
            vec![Particle {
                position: [2.0, 0.0, 0.0],
                velocity: [0.0; 3]
            }],
            options([0.0; 3])
        )
        .err(),
        Some(Error::Outside(0))
    );
    let mut volume = Volume::new(
        vec![Particle {
            position: [0.0; 3],
            velocity: [0.0; 3],
        }],
        options([0.0; 3]),
    )?;
    assert_eq!(volume.step(f32::NAN), Err(Error::Step));
    assert_eq!(volume.step(0.1), Err(Error::Step));
    Ok(())
}

#[test]
fn sampled_bed_and_waterline_cells_constrain_the_volume() -> Result<(), Error> {
    let mut volume = Volume::new(
        vec![Particle {
            position: [0.0, -0.5, 0.0],
            velocity: [0.0, -1.0, 0.0],
        }],
        options([0.0, -9.81, 0.0]),
    )?;
    assert!(volume.set_floor_grid([-0.5, -0.5], 1.0, 1, 1, vec![0.0]));
    assert!(volume.particles()[0].position[1] >= 0.125);
    volume.step(0.01)?;
    assert!(volume.particles()[0].position[1] >= 0.125);
    assert!(!volume.set_floor_grid([0.0, 0.0], f32::NAN, 1, 1, vec![0.0]));
    Ok(())
}

#[test]
fn a_surface_impulse_reaches_the_entire_liquid_column() -> Result<(), Error> {
    let particles = [-0.5, 0.0, 0.5]
        .map(|y| Particle {
            position: [0.0, y, 0.0],
            velocity: [0.0; 3],
        })
        .to_vec();
    let mut volume = Volume::new(particles, options([0.0; 3]))?;
    assert_eq!(
        volume.impulse_column([0.0, 0.5, 0.0], 0.1, [0.0, 1.0, 0.0]),
        3
    );
    assert!(volume.particles().iter().all(|p| p.velocity[1] > 0.0));
    Ok(())
}

#[test]
fn a_coarse_surface_grid_impulse_reaches_its_nearest_particle() -> Result<(), Error> {
    let mut volume = Volume::new(
        vec![Particle {
            position: [0.5, 0.0, 0.5],
            velocity: [0.0; 3],
        }],
        options([0.0; 3]),
    )?;
    assert_eq!(volume.impulse_column([0.0; 3], 0.1, [0.0, 1.0, 0.0]), 1);
    assert!(volume.particles()[0].velocity[1] > 0.0);
    Ok(())
}
