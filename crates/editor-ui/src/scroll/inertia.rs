//! Bounded wheel momentum uses GPUI's analytic spring; elapsed time never depends on frame count.
use super::targets::Metrics;
use gpui_kit::{Pixels, Point, SpringConfig, SpringState, point, px};
use std::time::Instant;

/// Critical damping avoids overscroll; an 18 rad/s response retains a short, decelerating tail.
const SPRING: SpringConfig = SpringConfig::new(324., 36., 1.);
/// Stop imperceptible subpixel travel instead of repainting indefinitely at high refresh rates.
const EPSILON: f32 = 0.25;

/// Only scroll coordinates and velocity are retained; native handles still own viewport state.
#[derive(Default)]
pub(super) struct Inertia {
    /// Accumulated, bounded wheel destination in logical pixels.
    target: Point<Pixels>,
    /// Horizontal logical position and logical pixels per second.
    x: SpringState,
    /// Vertical logical position and logical pixels per second.
    y: SpringState,
    /// Last integration time, including physical input received between native paints.
    updated: Option<Instant>,
    /// Only unsettled motion may ask the native platform for another vsync.
    active: bool,
}

impl Inertia {
    /// Whether either axis still has visible travel or velocity.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Stop without writing the native handle; the next event starts from its actual position.
    pub fn cancel(&mut self) {
        self.active = false;
        self.updated = None;
    }

    /// Merge a physical `delta` at `now`; return whether this viewport can consume wheel motion.
    pub fn push(&mut self, metrics: Metrics, delta: Point<Pixels>, now: Instant) -> bool {
        if !f32::from(delta.x).is_finite() || !f32::from(delta.y).is_finite() {
            return false;
        }
        if self.active {
            self.advance(metrics, now);
        } else {
            self.x = Self::rest(metrics.offset.x);
            self.y = Self::rest(metrics.offset.y);
            self.target = metrics.offset;
        }
        let old = self.target;
        // Reverse input brakes only that axis. It must not spend another gesture travelling
        // through an old endpoint; the other axis keeps its independent momentum.
        let axis = |state: &mut SpringState, actual: Pixels, target: Pixels, delta: Pixels| {
            if f32::from(target - actual) * f32::from(delta) < 0.
                || state.velocity * f32::from(delta) < 0.
            {
                *state = Self::rest(actual);
                actual + delta
            } else {
                target + delta
            }
        };
        self.target = metrics.clamp(point(
            axis(&mut self.x, metrics.offset.x, old.x, delta.x),
            axis(&mut self.y, metrics.offset.y, old.y, delta.y),
        ));
        self.updated = Some(now);
        self.active =
            self.position() != self.target || self.x.velocity != 0. || self.y.velocity != 0.;
        self.active
    }

    /// Sample both axes at the actual frame time and clamp against the current native bounds.
    pub fn advance(&mut self, metrics: Metrics, now: Instant) -> Point<Pixels> {
        if !self.active {
            return metrics.offset;
        }
        let elapsed = self
            .updated
            .replace(now)
            .map_or(0., |last| now.saturating_duration_since(last).as_secs_f32());
        self.target = metrics.clamp(self.target);
        self.x = Self::step(self.x, self.target.x, elapsed);
        self.y = Self::step(self.y, self.target.y, elapsed);
        let requested = self.position();
        let bounded = metrics.clamp(requested);
        if requested.x != bounded.x {
            self.x = Self::rest(bounded.x);
            self.target.x = bounded.x;
        }
        if requested.y != bounded.y {
            self.y = Self::rest(bounded.y);
            self.target.y = bounded.y;
        }
        self.active =
            self.position() != self.target || self.x.velocity != 0. || self.y.velocity != 0.;
        bounded
    }

    /// Exact spring position before native presentation aligns its vertical offset to pixels.
    pub(super) fn position(&self) -> Point<Pixels> {
        point(px(self.x.position), px(self.y.position))
    }

    /// Initialize a stopped axis from the native `position` after interruption or reaching a bound.
    fn rest(position: Pixels) -> SpringState {
        SpringState {
            position: f32::from(position),
            velocity: 0.,
        }
    }

    /// Reuse the upstream analytic propagator and settle both displacement and velocity.
    fn step(state: SpringState, target: Pixels, elapsed: f32) -> SpringState {
        let next = SPRING.step(state, f32::from(target), elapsed);
        if SPRING.is_settled(next, f32::from(target), EPSILON) {
            Self::rest(target)
        } else {
            next
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Known native bounds keep the same input meaningful across frame rates.
    fn metrics() -> Metrics {
        Metrics {
            offset: point(px(-200.), px(-200.)),
            max: Some(point(px(2000.), px(2000.))),
            line_height: None,
        }
    }

    /// Compare the same released gesture at 60/120/240 Hz and after a stalled native frame.
    #[test]
    fn inertia_is_independent_of_refresh_rate_and_stalls() {
        let start = Instant::now();
        let mut results = Vec::new();
        for frames in [1, 12, 24, 48] {
            let mut motion = Inertia::default();
            motion.push(metrics(), point(px(0.), px(-300.)), start);
            let mut offset = metrics().offset;
            for frame in 1..=frames {
                offset = motion.advance(
                    metrics(),
                    start + Duration::from_secs_f64(0.2 * frame as f64 / frames as f64),
                );
            }
            results.push(f32::from(offset.y));
        }
        assert!(
            results
                .iter()
                .all(|value| (value - results[0]).abs() < 0.001),
            "analytic travel differs by frame rate: {results:?}"
        );
    }

    /// Retargeting retains speed, while an opposite wheel brakes only the reversed axis.
    #[test]
    fn inertia_preserves_velocity_and_reversal_brakes_one_axis() {
        let start = Instant::now();
        let mut motion = Inertia::default();
        motion.push(metrics(), point(px(-100.), px(-100.)), start);
        let at = start + Duration::from_millis(45);
        let offset = motion.advance(metrics(), at);
        let velocity = motion.y.velocity;
        assert!(velocity < 0.);
        let current = Metrics {
            offset,
            ..metrics()
        };
        motion.push(current, point(px(0.), px(-100.)), at);
        assert_eq!(
            motion.y.velocity, velocity,
            "same-direction input carries velocity"
        );
        motion.push(current, point(px(40.), px(0.)), at);
        assert_eq!(motion.x.velocity, 0.);
        assert_eq!(
            motion.y.velocity, velocity,
            "horizontal braking preserves vertical inertia"
        );
        motion.advance(current, start + Duration::from_secs(2));
        assert!(
            !motion.active(),
            "the inertial tail must stop requesting frames"
        );
    }

    /// Shrinking bounds and malformed input cannot create overscroll, NaNs or perpetual motion.
    #[test]
    fn inertia_clamps_bursts_and_stops_at_current_bounds() {
        let start = Instant::now();
        let mut motion = Inertia::default();
        for _ in 0..120 {
            motion.push(metrics(), point(px(0.), px(-60.)), start);
        }
        let smaller = Metrics {
            max: Some(point(px(300.), px(300.))),
            ..metrics()
        };
        let final_offset = motion.advance(smaller, start + Duration::from_secs(2));
        assert_eq!(final_offset.y, px(-300.));
        assert!(!motion.active());
        assert!(!motion.push(smaller, point(px(f32::NAN), px(-3.)), start));
        motion.cancel();
        assert!(!motion.active());
    }
}
