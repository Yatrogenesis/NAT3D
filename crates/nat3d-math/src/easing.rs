/*
 * NAT3D - Next-generation Advanced Technology for 3D
 * Professional 3D Modeling, CAD, Physics Simulation and Rendering Suite
 *
 * Copyright (C) 2023-2026 Francisco Molina <pako.molina@gmail.com>
 *
 * This software is dual-licensed:
 * 1. Open Source: GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)
 * 2. Commercial: For commercial use, please contact <fmolina@avermex.com>
 *
 * For research information, visit: https://research.avermex.com
 * For collaborations, contact: <pako.molina@gmail.com>
 *
 * DOI: [PENDING]
 */

//! Easing functions for animation.
//!
//! Standard easing curves commonly used in animation and motion design.

use std::f64::consts::PI;

/// Easing function type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingType {
    /// Linear interpolation.
    Linear,
    /// Quadratic ease in.
    QuadIn,
    /// Quadratic ease out.
    QuadOut,
    /// Quadratic ease in-out.
    QuadInOut,
    /// Cubic ease in.
    CubicIn,
    /// Cubic ease out.
    CubicOut,
    /// Cubic ease in-out.
    CubicInOut,
    /// Quartic ease in.
    QuartIn,
    /// Quartic ease out.
    QuartOut,
    /// Quartic ease in-out.
    QuartInOut,
    /// Quintic ease in.
    QuintIn,
    /// Quintic ease out.
    QuintOut,
    /// Quintic ease in-out.
    QuintInOut,
    /// Sine ease in.
    SineIn,
    /// Sine ease out.
    SineOut,
    /// Sine ease in-out.
    SineInOut,
    /// Exponential ease in.
    ExpoIn,
    /// Exponential ease out.
    ExpoOut,
    /// Exponential ease in-out.
    ExpoInOut,
    /// Circular ease in.
    CircIn,
    /// Circular ease out.
    CircOut,
    /// Circular ease in-out.
    CircInOut,
    /// Elastic ease in.
    ElasticIn,
    /// Elastic ease out.
    ElasticOut,
    /// Elastic ease in-out.
    ElasticInOut,
    /// Back ease in (overshoot).
    BackIn,
    /// Back ease out (overshoot).
    BackOut,
    /// Back ease in-out (overshoot).
    BackInOut,
    /// Bounce ease in.
    BounceIn,
    /// Bounce ease out.
    BounceOut,
    /// Bounce ease in-out.
    BounceInOut,
}

/// Apply easing function to a value in [0, 1].
#[must_use]
pub fn ease(t: f64, easing: EasingType) -> f64 {
    let t = t.clamp(0.0, 1.0);

    match easing {
        EasingType::Linear => t,
        EasingType::QuadIn => quad_in(t),
        EasingType::QuadOut => quad_out(t),
        EasingType::QuadInOut => quad_in_out(t),
        EasingType::CubicIn => cubic_in(t),
        EasingType::CubicOut => cubic_out(t),
        EasingType::CubicInOut => cubic_in_out(t),
        EasingType::QuartIn => quart_in(t),
        EasingType::QuartOut => quart_out(t),
        EasingType::QuartInOut => quart_in_out(t),
        EasingType::QuintIn => quint_in(t),
        EasingType::QuintOut => quint_out(t),
        EasingType::QuintInOut => quint_in_out(t),
        EasingType::SineIn => sine_in(t),
        EasingType::SineOut => sine_out(t),
        EasingType::SineInOut => sine_in_out(t),
        EasingType::ExpoIn => expo_in(t),
        EasingType::ExpoOut => expo_out(t),
        EasingType::ExpoInOut => expo_in_out(t),
        EasingType::CircIn => circ_in(t),
        EasingType::CircOut => circ_out(t),
        EasingType::CircInOut => circ_in_out(t),
        EasingType::ElasticIn => elastic_in(t),
        EasingType::ElasticOut => elastic_out(t),
        EasingType::ElasticInOut => elastic_in_out(t),
        EasingType::BackIn => back_in(t),
        EasingType::BackOut => back_out(t),
        EasingType::BackInOut => back_in_out(t),
        EasingType::BounceIn => bounce_in(t),
        EasingType::BounceOut => bounce_out(t),
        EasingType::BounceInOut => bounce_in_out(t),
    }
}

// Quadratic easing

/// Quadratic ease in.
#[inline]
#[must_use]
pub fn quad_in(t: f64) -> f64 {
    t * t
}

/// Quadratic ease out.
#[inline]
#[must_use]
pub fn quad_out(t: f64) -> f64 {
    t * (2.0 - t)
}

/// Quadratic ease in-out.
#[inline]
#[must_use]
pub fn quad_in_out(t: f64) -> f64 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        -1.0 + (4.0 - 2.0 * t) * t
    }
}

// Cubic easing

/// Cubic ease in.
#[inline]
#[must_use]
pub fn cubic_in(t: f64) -> f64 {
    t * t * t
}

/// Cubic ease out.
#[inline]
#[must_use]
pub fn cubic_out(t: f64) -> f64 {
    let t = t - 1.0;
    t * t * t + 1.0
}

/// Cubic ease in-out.
#[inline]
#[must_use]
pub fn cubic_in_out(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let t = 2.0 * t - 2.0;
        0.5 * t * t * t + 1.0
    }
}

// Quartic easing

/// Quartic ease in.
#[inline]
#[must_use]
pub fn quart_in(t: f64) -> f64 {
    t * t * t * t
}

/// Quartic ease out.
#[inline]
#[must_use]
pub fn quart_out(t: f64) -> f64 {
    let t = t - 1.0;
    1.0 - t * t * t * t
}

/// Quartic ease in-out.
#[inline]
#[must_use]
pub fn quart_in_out(t: f64) -> f64 {
    if t < 0.5 {
        8.0 * t * t * t * t
    } else {
        let t = t - 1.0;
        1.0 - 8.0 * t * t * t * t
    }
}

// Quintic easing

/// Quintic ease in.
#[inline]
#[must_use]
pub fn quint_in(t: f64) -> f64 {
    t * t * t * t * t
}

/// Quintic ease out.
#[inline]
#[must_use]
pub fn quint_out(t: f64) -> f64 {
    let t = t - 1.0;
    t * t * t * t * t + 1.0
}

/// Quintic ease in-out.
#[inline]
#[must_use]
pub fn quint_in_out(t: f64) -> f64 {
    if t < 0.5 {
        16.0 * t * t * t * t * t
    } else {
        let t = 2.0 * t - 2.0;
        0.5 * t * t * t * t * t + 1.0
    }
}

// Sine easing

/// Sine ease in.
#[inline]
#[must_use]
pub fn sine_in(t: f64) -> f64 {
    1.0 - (t * PI / 2.0).cos()
}

/// Sine ease out.
#[inline]
#[must_use]
pub fn sine_out(t: f64) -> f64 {
    (t * PI / 2.0).sin()
}

/// Sine ease in-out.
#[inline]
#[must_use]
pub fn sine_in_out(t: f64) -> f64 {
    0.5 * (1.0 - (PI * t).cos())
}

// Exponential easing

/// Exponential ease in.
#[inline]
#[must_use]
pub fn expo_in(t: f64) -> f64 {
    if t == 0.0 {
        0.0
    } else {
        2.0_f64.powf(10.0 * (t - 1.0))
    }
}

/// Exponential ease out.
#[inline]
#[must_use]
pub fn expo_out(t: f64) -> f64 {
    if t == 1.0 {
        1.0
    } else {
        1.0 - 2.0_f64.powf(-10.0 * t)
    }
}

/// Exponential ease in-out.
#[inline]
#[must_use]
pub fn expo_in_out(t: f64) -> f64 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    if t < 0.5 {
        0.5 * 2.0_f64.powf(20.0 * t - 10.0)
    } else {
        1.0 - 0.5 * 2.0_f64.powf(-20.0 * t + 10.0)
    }
}

// Circular easing

/// Circular ease in.
#[inline]
#[must_use]
pub fn circ_in(t: f64) -> f64 {
    1.0 - (1.0 - t * t).sqrt()
}

/// Circular ease out.
#[inline]
#[must_use]
pub fn circ_out(t: f64) -> f64 {
    let t = t - 1.0;
    (1.0 - t * t).sqrt()
}

/// Circular ease in-out.
#[inline]
#[must_use]
pub fn circ_in_out(t: f64) -> f64 {
    if t < 0.5 {
        0.5 * (1.0 - (1.0 - 4.0 * t * t).sqrt())
    } else {
        let t = 2.0 * t - 2.0;
        0.5 * ((1.0 - t * t).sqrt() + 1.0)
    }
}

// Elastic easing

const C4: f64 = 2.0 * PI / 3.0;
const C5: f64 = 2.0 * PI / 4.5;

/// Elastic ease in.
#[inline]
#[must_use]
pub fn elastic_in(t: f64) -> f64 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    -2.0_f64.powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * C4).sin()
}

/// Elastic ease out.
#[inline]
#[must_use]
pub fn elastic_out(t: f64) -> f64 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    2.0_f64.powf(-10.0 * t) * ((t * 10.0 - 0.75) * C4).sin() + 1.0
}

/// Elastic ease in-out.
#[inline]
#[must_use]
pub fn elastic_in_out(t: f64) -> f64 {
    if t == 0.0 {
        return 0.0;
    }
    if t == 1.0 {
        return 1.0;
    }
    if t < 0.5 {
        -0.5 * 2.0_f64.powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * C5).sin()
    } else {
        0.5 * 2.0_f64.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * C5).sin() + 1.0
    }
}

// Back easing (overshoot)

const C1: f64 = 1.70158;
const C2: f64 = C1 * 1.525;
const C3: f64 = C1 + 1.0;

/// Back ease in (with overshoot).
#[inline]
#[must_use]
pub fn back_in(t: f64) -> f64 {
    C3 * t * t * t - C1 * t * t
}

/// Back ease out (with overshoot).
#[inline]
#[must_use]
pub fn back_out(t: f64) -> f64 {
    let t = t - 1.0;
    1.0 + C3 * t * t * t + C1 * t * t
}

/// Back ease in-out (with overshoot).
#[inline]
#[must_use]
pub fn back_in_out(t: f64) -> f64 {
    if t < 0.5 {
        let t2 = 2.0 * t;
        (t2 * t2 * ((C2 + 1.0) * t2 - C2)) / 2.0
    } else {
        let t2 = 2.0 * t - 2.0;
        (t2 * t2 * ((C2 + 1.0) * t2 + C2) + 2.0) / 2.0
    }
}

// Bounce easing

/// Bounce ease out (base function).
#[inline]
#[must_use]
pub fn bounce_out(t: f64) -> f64 {
    const N1: f64 = 7.5625;
    const D1: f64 = 2.75;

    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        let t = t - 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        let t = t - 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        let t = t - 2.625 / D1;
        N1 * t * t + 0.984375
    }
}

/// Bounce ease in.
#[inline]
#[must_use]
pub fn bounce_in(t: f64) -> f64 {
    1.0 - bounce_out(1.0 - t)
}

/// Bounce ease in-out.
#[inline]
#[must_use]
pub fn bounce_in_out(t: f64) -> f64 {
    if t < 0.5 {
        (1.0 - bounce_out(1.0 - 2.0 * t)) / 2.0
    } else {
        (1.0 + bounce_out(2.0 * t - 1.0)) / 2.0
    }
}

/// Bezier-based easing (CSS cubic-bezier).
/// Control points: (0,0), (x1,y1), (x2,y2), (1,1)
#[must_use]
pub fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, t: f64) -> f64 {
    // Find t for x using Newton's method
    let mut guess = t;
    for _ in 0..8 {
        let x = bezier_sample(x1, x2, guess);
        let dx = bezier_derivative(x1, x2, guess);
        if dx.abs() < 1e-10 {
            break;
        }
        guess -= (x - t) / dx;
    }

    bezier_sample(y1, y2, guess)
}

#[inline]
fn bezier_sample(p1: f64, p2: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    3.0 * p1 * t * (1.0 - t) * (1.0 - t) + 3.0 * p2 * t2 * (1.0 - t) + t3
}

#[inline]
fn bezier_derivative(p1: f64, p2: f64, t: f64) -> f64 {
    let mt = 1.0 - t;
    3.0 * p1 * mt * mt + 6.0 * (p2 - p1) * mt * t + 3.0 * (1.0 - p2) * t * t
}

/// Common CSS easing presets.
pub mod presets {
    use super::*;

    /// CSS ease
    pub fn css_ease(t: f64) -> f64 {
        cubic_bezier(0.25, 0.1, 0.25, 1.0, t)
    }

    /// CSS ease-in
    pub fn css_ease_in(t: f64) -> f64 {
        cubic_bezier(0.42, 0.0, 1.0, 1.0, t)
    }

    /// CSS ease-out
    pub fn css_ease_out(t: f64) -> f64 {
        cubic_bezier(0.0, 0.0, 0.58, 1.0, t)
    }

    /// CSS ease-in-out
    pub fn css_ease_in_out(t: f64) -> f64 {
        cubic_bezier(0.42, 0.0, 0.58, 1.0, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear() {
        assert!((ease(0.0, EasingType::Linear) - 0.0).abs() < 1e-10);
        assert!((ease(0.5, EasingType::Linear) - 0.5).abs() < 1e-10);
        assert!((ease(1.0, EasingType::Linear) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_quad() {
        assert!((quad_in(0.0) - 0.0).abs() < 1e-10);
        assert!((quad_in(1.0) - 1.0).abs() < 1e-10);
        assert!((quad_out(0.0) - 0.0).abs() < 1e-10);
        assert!((quad_out(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cubic() {
        assert!((cubic_in(0.0) - 0.0).abs() < 1e-10);
        assert!((cubic_in(1.0) - 1.0).abs() < 1e-10);
        assert!((cubic_out(0.0) - 0.0).abs() < 1e-10);
        assert!((cubic_out(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_elastic() {
        assert!((elastic_in(0.0) - 0.0).abs() < 1e-10);
        assert!((elastic_in(1.0) - 1.0).abs() < 1e-10);
        assert!((elastic_out(0.0) - 0.0).abs() < 1e-10);
        assert!((elastic_out(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_bounce() {
        assert!((bounce_out(0.0) - 0.0).abs() < 1e-10);
        assert!((bounce_out(1.0) - 1.0).abs() < 1e-10);
        assert!((bounce_in(0.0) - 0.0).abs() < 1e-10);
        assert!((bounce_in(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_in_out_symmetry() {
        // In-out functions should be symmetric around 0.5
        assert!((quad_in_out(0.5) - 0.5).abs() < 1e-10);
        assert!((cubic_in_out(0.5) - 0.5).abs() < 1e-10);
        assert!((sine_in_out(0.5) - 0.5).abs() < 1e-10);
    }
}
