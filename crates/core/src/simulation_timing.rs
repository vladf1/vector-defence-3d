//! The bounded substep policy: native high-refresh deltas are preserved, slow frames are split
//! into steps of at most 1/60 second, and catch-up work is capped.
pub const MAX_SIMULATION_STEP_SECONDS: f64 = 1.0 / 60.0;
pub const MAX_SIMULATION_BACKLOG_SECONDS: f64 = 0.5;
pub const MAX_SIMULATION_SUBSTEPS_PER_FRAME: u32 = 8;

const SIMULATION_TIME_EPSILON_SECONDS: f64 = 1e-9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundedSimulationSubstepResult {
    pub dropped_seconds: f64,
    pub remaining_seconds: f64,
    pub simulated_seconds: f64,
    pub step_count: u32,
}

/// Runs `simulate` in steps of at most `MAX_SIMULATION_STEP_SECONDS`; it returns whether the
/// simulation wants to keep running (false stops after that step).
pub fn run_bounded_simulation_substeps(
    available_seconds: f64,
    mut simulate: impl FnMut(f64) -> bool,
) -> BoundedSimulationSubstepResult {
    let non_negative_seconds = available_seconds.max(0.0);
    let mut remaining_seconds = non_negative_seconds.min(MAX_SIMULATION_BACKLOG_SECONDS);
    let dropped_seconds = non_negative_seconds - remaining_seconds;
    let mut simulated_seconds = 0.0;
    let mut step_count = 0;

    while remaining_seconds > SIMULATION_TIME_EPSILON_SECONDS && step_count < MAX_SIMULATION_SUBSTEPS_PER_FRAME {
        let delta_seconds = remaining_seconds.min(MAX_SIMULATION_STEP_SECONDS);
        let should_continue = simulate(delta_seconds);
        remaining_seconds = (remaining_seconds - delta_seconds).max(0.0);
        simulated_seconds += delta_seconds;
        step_count += 1;
        if !should_continue {
            break;
        }
    }

    if remaining_seconds <= SIMULATION_TIME_EPSILON_SECONDS {
        remaining_seconds = 0.0;
    }

    BoundedSimulationSubstepResult { dropped_seconds, remaining_seconds, simulated_seconds, step_count }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-7
    }

    #[test]
    fn bounded_steps_conserve_time() {
        for seconds in [0.0, -1.0, 1.0 / 240.0, 1.0 / 60.0, 0.1, 2.0] {
            let mut steps = Vec::new();
            let timing = run_bounded_simulation_substeps(seconds, |delta| {
                steps.push(delta);
                true
            });
            assert!(steps.len() <= 8);
            assert!(steps.iter().all(|&delta| delta <= 1.0 / 60.0));
            assert!(near(
                timing.simulated_seconds + timing.remaining_seconds + timing.dropped_seconds,
                seconds.max(0.0)
            ));
        }
    }

    #[test]
    fn substeps_stop_when_the_simulation_stops() {
        let stopped = run_bounded_simulation_substeps(0.1, |_| false);
        assert_eq!(stopped.step_count, 1);
        assert!(near(stopped.remaining_seconds, 0.1 - 1.0 / 60.0));
    }
}
