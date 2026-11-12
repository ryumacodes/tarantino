//! Source-space magnifier targets. Rendering and animation consume these values
//! independently of the whole-screen camera transform.
use super::source_crop::SourceCrop;
use crate::cursor_renderer::SpringConfig;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub mod camera_overlap;
pub mod camera_opacity;
pub mod camera_opacity_prepare;
pub mod timeline;
pub mod animation;
pub mod prepare;
pub mod content_pass;
pub mod projection;
pub mod mip_pyramid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct LoupeStyle {
    pub radius: f64,
    pub glass: bool,
    pub bevel: f64,
    pub chromatic_aberration: f64,
    pub shadow_alpha: f64,
}

impl LoupeStyle {
    pub fn options(self, zoom: f64) -> LoupeOptions {
        LoupeOptions { radius: self.radius, zoom, glass: self.glass, bevel: self.bevel,
            chromatic_aberration: self.chromatic_aberration, shadow_alpha: self.shadow_alpha }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LoupeOptions {
    /// Radius relative to the longer visible source dimension.
    pub radius: f64,
    pub zoom: f64,
    pub glass: bool,
    pub bevel: f64,
    pub chromatic_aberration: f64,
    pub shadow_alpha: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct LoupeTarget {
    /// Original source-normalized center; radius uses source width as its unit.
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub zoom: f64,
    pub bevel: f64,
    pub chromatic_aberration: f64,
    pub shadow_alpha: f64,
    pub presence: f64,
}

impl LoupeTarget {
    /// Retain the nearest range's geometry while its optical effect fades out.
    pub fn inactive(self) -> Self {
        Self {
            zoom: 1.0,
            bevel: 0.0,
            chromatic_aberration: 0.0,
            shadow_alpha: 0.0,
            presence: 0.0,
            ..self
        }
    }
}

/// A deterministic target can be sampled in any order, including backward seeks.
/// Focus is relative to the crop, before whole-screen camera projection.
pub fn target_at(
    source: (u32, u32),
    crop: SourceCrop,
    focus: (f64, f64),
    source_time_ms: f64,
    options: LoupeOptions,
) -> Result<LoupeTarget> {
    let crop = crop.geometry(source.0, source.1)?;
    if ![
        focus.0,
        focus.1,
        source_time_ms,
        options.radius,
        options.zoom,
        options.bevel,
        options.chromatic_aberration,
        options.shadow_alpha,
    ]
    .iter()
    .all(|value| value.is_finite())
        || source_time_ms < 0.0
        || options.radius <= 0.0
        || options.zoom < 1.0
        || options.bevel < 0.0
        || options.chromatic_aberration < 0.0
        || !(0.0..=1.0).contains(&options.shadow_alpha)
    {
        bail!("Invalid magnifier geometry or optical settings");
    }
    let width = source.0 as f64;
    let height = source.1 as f64;
    let (drift_x, drift_y) = drift(source_time_ms);
    Ok(LoupeTarget {
        center_x: (crop.x as f64 + focus.0.clamp(0.0, 1.0) * crop.width as f64) / width + drift_x,
        center_y: (crop.y as f64 + focus.1.clamp(0.0, 1.0) * crop.height as f64) / height + drift_y,
        radius: options.radius * crop.width.max(crop.height) as f64 / width,
        zoom: options.zoom,
        bevel: if options.glass { options.bevel } else { 0.0 },
        chromatic_aberration: if options.glass {
            options.chromatic_aberration
        } else {
            0.0
        },
        shadow_alpha: options.shadow_alpha,
        presence: 1.0,
    })
}

fn drift(time_ms: f64) -> (f64, f64) {
    let tau = std::f64::consts::TAU;
    (
        (time_ms / 12000.0 * tau).sin() * 0.0125,
        ((time_ms + 7500.0) / 18000.0 * tau).cos() * 0.0125,
    )
}

pub fn movement_spring(screen: SpringConfig) -> SpringConfig {
    if screen.tension == 0.0 || screen.mass == 0.0 {
        return screen;
    }
    SpringConfig {
        tension: screen.tension * 1.5,
        ..screen
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> LoupeOptions {
        LoupeOptions {
            radius: 0.25,
            zoom: 2.0,
            glass: true,
            bevel: 0.1,
            chromatic_aberration: 0.02,
            shadow_alpha: 0.35,
        }
    }
    fn full() -> SourceCrop {
        SourceCrop {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }

    #[test]
    fn radius_stays_circular_in_portrait_and_cropped_sources() {
        let landscape = target_at((1920, 1080), full(), (0.5, 0.5), 0.0, options()).unwrap();
        let portrait = target_at((1080, 1920), full(), (0.5, 0.5), 0.0, options()).unwrap();
        assert_eq!(landscape.radius * 1920.0, 480.0);
        assert!((portrait.radius * 1080.0 - 480.0).abs() < 1e-9);
        let crop = SourceCrop {
            x: 0.25,
            y: 0.0,
            width: 0.5,
            height: 1.0,
        };
        let cropped = target_at((1920, 1080), crop, (0.0, 0.5), 0.0, options()).unwrap();
        assert_eq!(cropped.radius * 1920.0, 270.0);
        assert_eq!(cropped.center_x, 0.25);
    }

    #[test]
    fn drift_is_source_timed_and_focus_is_clamped_before_drift() {
        let sample = |time| target_at((100, 100), full(), (-1.0, 2.0), time, options()).unwrap();
        let before = sample(3000.0);
        assert!((before.center_x - 0.0125).abs() < 1e-12);
        assert!((sample(9000.0).center_x + 0.0125).abs() < 1e-12);
        assert_eq!(sample(3000.0), before);
        assert!((before.center_y - (1.0 + drift(3000.0).1)).abs() < 1e-12);
    }

    #[test]
    fn inactive_target_keeps_geometry_but_removes_optics() {
        let target = target_at((1920, 1080), full(), (0.3, 0.7), 1000.0, options()).unwrap();
        let inactive = target.inactive();
        assert_eq!(
            (inactive.center_x, inactive.center_y, inactive.radius),
            (target.center_x, target.center_y, target.radius)
        );
        assert_eq!(
            (inactive.zoom, inactive.presence, inactive.shadow_alpha),
            (1.0, 0.0, 0.0)
        );
        assert_eq!((inactive.bevel, inactive.chromatic_aberration), (0.0, 0.0));
        let flat = target_at(
            (1920, 1080),
            full(),
            (0.3, 0.7),
            1000.0,
            LoupeOptions {
                glass: false,
                ..options()
            },
        )
        .unwrap();
        assert_eq!((flat.bevel, flat.chromatic_aberration), (0.0, 0.0));
        assert_eq!(
            (flat.radius, flat.zoom, flat.shadow_alpha),
            (target.radius, 2.0, 0.35)
        );
    }

    #[test]
    fn loupe_spring_preserves_damping_mass_and_instant_modes() {
        let spring = SpringConfig {
            tension: 100.0,
            friction: 20.0,
            mass: 2.0,
        };
        let derived = movement_spring(spring);
        assert_eq!(
            (derived.tension, derived.friction, derived.mass),
            (150.0, 20.0, 2.0)
        );
        assert_eq!(
            movement_spring(SpringConfig {
                mass: 0.0,
                ..spring
            })
            .tension,
            100.0
        );
        assert_eq!(
            movement_spring(SpringConfig {
                tension: 0.0,
                ..spring
            })
            .tension,
            0.0
        );
    }

    #[test]
    fn invalid_inputs_do_not_reach_the_renderer() {
        for bad in [f64::NAN, f64::INFINITY, -1.0] {
            assert!(target_at((100, 100), full(), (0.5, 0.5), bad, options()).is_err());
            assert!(target_at(
                (100, 100),
                full(),
                (0.5, 0.5),
                0.0,
                LoupeOptions {
                    radius: bad,
                    ..options()
                }
            )
            .is_err());
        }
        assert!(target_at((0, 100), full(), (0.5, 0.5), 0.0, options()).is_err());
    }
}
