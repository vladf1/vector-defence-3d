//! Per-phase renderer startup timings.
use crate::shaders::{push_float, push_integer};

/// Milliseconds spent in each startup phase, for diagnosing slow devices.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StartupTimings {
    /// Until the GPU device is usable (the page hands in a ready device, so this is just the
    /// canvas configuration).
    pub device_ms: f64,
    /// Procedural meshes (CPU only), built while the pipelines compile.
    pub geometry_ms: f64,
    pub setup_ms: f64,
    pub pipeline_ms: f64,
    pub warmup_frame_ms: f64,
    pub gpu_drain_ms: f64,
    pub total_ms: f64,
    pub pipelines: u32,
}

impl StartupTimings {
    /// The TypeScript `StartupTimings` object as JSON (camelCase keys).
    pub fn to_json(&self) -> String {
        let mut out = String::with_capacity(192);
        let fields = [
            ("{\"deviceMs\":", self.device_ms),
            (",\"geometryMs\":", self.geometry_ms),
            (",\"setupMs\":", self.setup_ms),
            (",\"pipelineMs\":", self.pipeline_ms),
            (",\"warmupFrameMs\":", self.warmup_frame_ms),
            (",\"gpuDrainMs\":", self.gpu_drain_ms),
            (",\"totalMs\":", self.total_ms),
        ];
        for (key, value) in fields {
            out.push_str(key);
            push_float(&mut out, if value.is_finite() { value } else { 0.0 });
        }
        out.push_str(",\"pipelines\":");
        push_integer(&mut out, self.pipelines as f64);
        out.push('}');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_camel_case_keys() {
        let timings = StartupTimings {
            device_ms: 0.0,
            geometry_ms: 3.25,
            setup_ms: 4.5,
            pipeline_ms: 120.0,
            warmup_frame_ms: 2.125,
            gpu_drain_ms: 16.4,
            total_ms: 146.3,
            pipelines: 8,
        };
        assert_eq!(
            timings.to_json(),
            "{\"deviceMs\":0.0,\"geometryMs\":3.25,\"setupMs\":4.5,\"pipelineMs\":120.0,\"warmupFrameMs\":2.125,\
             \"gpuDrainMs\":16.4,\"totalMs\":146.3,\"pipelines\":8}"
        );
    }
}
