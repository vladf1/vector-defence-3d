//! Startup-fixed quality budgets and the dynamic-resolution governor.
use vd_core::profile::{GameMode, GameProfile};

/// Startup-fixed rendering budget. Anything that would change shader programs (MSAA,
/// light count) is decided once here; only resolution adapts at runtime.
#[derive(Clone, Copy, Debug)]
pub struct RenderQuality {
    pub max_pixel_ratio: f64,
    pub min_pixel_ratio: f64,
    /// Upper bound on rendered pixels, so huge high-DPR windows don't render far beyond need.
    pub max_render_pixels: f64,
    pub msaa_samples: u32,
    pub bloom_resolution: f64,
    pub flash_lights: usize,
    pub fx_particles: usize,
    pub glow_sprites: usize,
    pub smoke_sprites: usize,
    pub ribbons: usize,
}

const DESKTOP_QUALITY: RenderQuality = RenderQuality {
    max_pixel_ratio: 2.0,
    min_pixel_ratio: 0.85,
    max_render_pixels: 6_000_000.0,
    msaa_samples: 4,
    bloom_resolution: 0.5,
    flash_lights: 4,
    fx_particles: 1400,
    glow_sprites: 4096,
    smoke_sprites: 1024,
    ribbons: 2048,
};

const MOBILE_QUALITY: RenderQuality = RenderQuality {
    max_pixel_ratio: 2.0,
    min_pixel_ratio: 1.0,
    max_render_pixels: 2_600_000.0,
    msaa_samples: 0,
    bloom_resolution: 0.35,
    flash_lights: 2,
    fx_particles: 800,
    glow_sprites: 3072,
    smoke_sprites: 640,
    ribbons: 1536,
};

pub fn select_render_quality(profile: &GameProfile) -> RenderQuality {
    if profile.mode == GameMode::Mobile { MOBILE_QUALITY } else { DESKTOP_QUALITY }
}

const SLOW_FRAME_MS: f64 = 19.5;
const FAST_FRAME_MS: f64 = 17.6;
const EVALUATION_WINDOW_MS: f64 = 800.0;
const UPGRADE_PATIENCE_MS: f64 = 5000.0;
const CEILING_RECOVERY_MS: f64 = 20000.0;
const STEP_DOWN_RATIO: f64 = 0.84;
const STEP_UP_RATIO: f64 = 1.1;
const MAX_TRACKED_INTERVAL_MS: f64 = 100.0;

/// Adapts the render pixel ratio to keep frames near 60 fps. It steps down quickly when
/// frames run long and probes back up slowly; a ratio that proved too slow becomes a
/// ceiling that only relaxes after a long stable stretch.
#[derive(Clone, Debug)]
pub struct ResolutionGovernor {
    quality: RenderQuality,
    device_pixel_ratio: f64,
    pixel_budget_ratio: f64,
    pixel_ratio: f64,
    ceiling: f64,
    ceiling_set_at: f64,
    window_start: f64,
    window_frames: u32,
    window_total_ms: f64,
    fast_since: f64,
    last_frame_time: f64,
}

impl ResolutionGovernor {
    pub fn new(quality: RenderQuality, device_pixel_ratio: f64) -> Self {
        let mut governor = ResolutionGovernor {
            quality,
            device_pixel_ratio,
            pixel_budget_ratio: f64::INFINITY,
            pixel_ratio: 1.0,
            ceiling: 1.0,
            ceiling_set_at: 0.0,
            window_start: 0.0,
            window_frames: 0,
            window_total_ms: 0.0,
            fast_since: 0.0,
            last_frame_time: 0.0,
        };
        governor.pixel_ratio = governor.max_ratio();
        governor.ceiling = governor.max_ratio();
        governor
    }

    pub fn current_pixel_ratio(&self) -> f64 {
        self.pixel_ratio
    }

    fn max_ratio(&self) -> f64 {
        self.quality.max_pixel_ratio.min(self.device_pixel_ratio).min(self.pixel_budget_ratio)
    }

    fn min_ratio(&self) -> f64 {
        self.quality.min_pixel_ratio.min(self.max_ratio())
    }

    /// Re-derives limits for a new CSS size and device pixel ratio.
    pub fn update_viewport(&mut self, css_width: f64, css_height: f64, device_pixel_ratio: f64) {
        self.device_pixel_ratio = device_pixel_ratio;
        self.pixel_budget_ratio = (self.quality.max_render_pixels / (css_width * css_height).max(1.0)).sqrt();
        // Resume at the best ratio not already proven too slow; a smaller window may allow more.
        self.pixel_ratio = self.min_ratio().max(self.ceiling.min(self.max_ratio()));
    }

    /// Records a rendered frame; returns true when the pixel ratio changed.
    pub fn record_frame(&mut self, now: f64, animating: bool) -> bool {
        let interval = now - self.last_frame_time;
        self.last_frame_time = now;
        if !animating || interval <= 0.0 || interval > MAX_TRACKED_INTERVAL_MS {
            self.reset_window(now);
            return false;
        }

        self.window_frames += 1;
        self.window_total_ms += interval;
        if now - self.window_start < EVALUATION_WINDOW_MS {
            return false;
        }

        let average = self.window_total_ms / self.window_frames as f64;
        self.reset_window(now);

        if average > SLOW_FRAME_MS {
            self.fast_since = 0.0;
            if self.pixel_ratio <= self.min_ratio() {
                return false;
            }
            self.ceiling = self.pixel_ratio * 0.97;
            self.ceiling_set_at = now;
            self.pixel_ratio = self.min_ratio().max(self.pixel_ratio * STEP_DOWN_RATIO);
            return true;
        }

        if average >= FAST_FRAME_MS {
            self.fast_since = 0.0;
            return false;
        }

        if self.fast_since == 0.0 {
            self.fast_since = now;
        }
        if now - self.ceiling_set_at >= CEILING_RECOVERY_MS {
            self.ceiling = self.max_ratio();
        }
        let target = self.ceiling.min(self.max_ratio()).min(self.pixel_ratio * STEP_UP_RATIO);
        if now - self.fast_since < UPGRADE_PATIENCE_MS || target <= self.pixel_ratio + 0.01 {
            return false;
        }
        self.pixel_ratio = target;
        self.fast_since = now;
        true
    }

    fn reset_window(&mut self, now: f64) {
        self.window_start = now;
        self.window_frames = 0;
        self.window_total_ms = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_down_on_slow_frames_and_back_up_later() {
        let mut governor = ResolutionGovernor::new(DESKTOP_QUALITY, 2.0);
        governor.update_viewport(1000.0, 700.0, 2.0);
        assert_eq!(governor.current_pixel_ratio(), 2.0);
        let mut now = 1000.0;
        let mut changed = false;
        for _ in 0..40 {
            now += 25.0;
            changed |= governor.record_frame(now, true);
        }
        assert!(changed && governor.current_pixel_ratio() < 2.0);
        let lowered = governor.current_pixel_ratio();
        for _ in 0..3000 {
            now += 16.0;
            governor.record_frame(now, true);
        }
        assert!(governor.current_pixel_ratio() > lowered);
    }

    #[test]
    fn idle_frames_never_change_the_ratio() {
        let mut governor = ResolutionGovernor::new(MOBILE_QUALITY, 3.0);
        let mut now = 0.0;
        for _ in 0..200 {
            now += 40.0;
            assert!(!governor.record_frame(now, false));
        }
    }
}
