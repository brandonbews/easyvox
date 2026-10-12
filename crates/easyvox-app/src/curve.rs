//! The overlay's timing curves: how far along an animation is (y) at each share of its time
//! (x), as the control points `[x1, y1, x2, y2]` of a cubic Bézier from (0, 0) to (1, 1), like
//! CSS's `cubic-bezier()`. GPUI's frames read them for the words and the glide, and the panel
//! samples them into Core Animation keyframes for the open and close.

/// A cubic Bézier timing curve's two middle control points, `[x1, y1, x2, y2]`.
pub type Curve = [f32; 4];

/// The open's growth. It passes full size at 37 % of the way through, peaks 9.8 % of the growth
/// beyond it at 57 %, and lands on it exactly at the end. So the pop overshoots a little and
/// still takes exactly the open's duration. A spring (`CASpringAnimation`) would settle in its
/// own time instead, whatever the duration says.
pub const POP: Curve = [0.34, 1.56, 0.64, 1.];
/// The open's fade, a quick ease-out (a quartic): 60 % opaque a fifth of the way through, 84 %
/// as the pill passes full size and 96 % at the pop's peak, so the overshoot is seen.
pub const FADE_IN: Curve = [0.25, 1., 0.5, 1.];
/// The close's fade and shrink: Core Animation's ease-in (`kCAMediaTimingFunctionEaseIn`).
pub const EASE_IN: Curve = [0.42, 0., 1., 1.];
/// New words fading in, the pill gliding and the meter's bars easing: CSS's `ease-out`. It
/// moves at once and slows steadily, a ninth of the way at most in a frame of a 250 ms glide at
/// 60 frames a second. (A cubic ease-out jumps in its first frame; CSS's `ease` starts so slowly
/// that new words sit cut off at the pill's edge.)
pub const EASE_OUT: Curve = [0., 0., 0.58, 1.];

/// How far along `curve` an animation is at `t`, its share of the time (0 to 1): the curve's y
/// where its x is `t`. Past 1 where the curve overshoots; exactly 0 and 1 at the ends.
pub fn along([x1, y1, x2, y2]: Curve, t: f64) -> f64 {
    if t <= 0. {
        return 0.;
    }
    if t >= 1. {
        return 1.;
    }
    let bezier = |a: f32, b: f32, u: f64| {
        let (a, b) = (f64::from(a), f64::from(b));
        3. * (1. - u).powi(2) * u * a + 3. * (1. - u) * u * u * b + u.powi(3)
    };
    // x only grows along the curve (x1 and x2 are within 0 to 1), so halve the way to `t`.
    let (mut lo, mut hi) = (0., 1.);
    for _ in 0..50 {
        let mid = (lo + hi) / 2.;
        if bezier(x1, x2, mid) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bezier(y1, y2, (lo + hi) / 2.)
}

#[cfg(test)]
mod tests {
    use super::{EASE_IN, EASE_OUT, POP, along};

    fn samples(curve: [f32; 4]) -> Vec<(f64, f64)> {
        (0..=1000).map(|i| (i as f64 / 1000., along(curve, i as f64 / 1000.))).collect()
    }

    #[test]
    fn the_pop_overshoots_a_little_and_lands_on_time() {
        assert_eq!((along(POP, 0.), along(POP, 1.)), (0., 1.));
        // Past full size from 37 % of the way, by at most 9.8 % of the growth, at 57 %.
        let s = samples(POP);
        let (at, peak) = s.iter().copied().fold((0., 0.), |best, p| if p.1 > best.1 { p } else { best });
        assert!((peak - 1.098).abs() < 0.001 && (at - 0.573).abs() < 0.01, "{peak} at {at}");
        let past = s.iter().find(|p| p.1 > 1.).unwrap().0;
        assert!((past - 0.369).abs() < 0.01, "{past}");
        // Back to full size without going under it again.
        assert!(s.iter().filter(|p| p.0 > past).all(|p| p.1 >= 1.));
    }

    #[test]
    fn the_other_curves_go_one_way_from_0_to_1() {
        for curve in [EASE_IN, EASE_OUT] {
            let s = samples(curve);
            assert!(s.windows(2).all(|w| w[1].1 >= w[0].1 && w[1].1 <= 1.), "{curve:?}");
        }
        // CSS's `ease-out` at two points, as browsers compute it.
        assert!((along(EASE_OUT, 0.25) - 0.3781).abs() < 0.001, "{}", along(EASE_OUT, 0.25));
        assert!((along(EASE_OUT, 0.5) - 0.6846).abs() < 0.001, "{}", along(EASE_OUT, 0.5));
    }

    #[test]
    fn a_glide_frame_never_jumps() {
        // At 60 frames a second, a 250 ms glide moves at most 11 % of the way in one frame.
        let frames: Vec<f64> = (0..=15).map(|i| along(EASE_OUT, (i as f64 * 16.7 / 250.).min(1.))).collect();
        let biggest = frames.windows(2).map(|w| w[1] - w[0]).fold(0., f64::max);
        assert!(biggest < 0.11, "{biggest}");
    }
}
