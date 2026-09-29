export const MAX_SIMULATION_STEP_SECONDS = 1 / 60;
export const MAX_SIMULATION_BACKLOG_SECONDS = 0.5;
export const MAX_SIMULATION_SUBSTEPS_PER_FRAME = 8;

const SIMULATION_TIME_EPSILON_SECONDS = 1e-9;

export interface BoundedSimulationSubstepResult {
  readonly droppedSeconds: number;
  readonly remainingSeconds: number;
  readonly simulatedSeconds: number;
  readonly stepCount: number;
}

export function runBoundedSimulationSubsteps(
  availableSeconds: number,
  simulate: (deltaSeconds: number) => boolean,
): BoundedSimulationSubstepResult {
  const nonNegativeSeconds = Math.max(0, availableSeconds);
  let remainingSeconds = Math.min(nonNegativeSeconds, MAX_SIMULATION_BACKLOG_SECONDS);
  const droppedSeconds = nonNegativeSeconds - remainingSeconds;
  let simulatedSeconds = 0;
  let stepCount = 0;

  while (
    remainingSeconds > SIMULATION_TIME_EPSILON_SECONDS
    && stepCount < MAX_SIMULATION_SUBSTEPS_PER_FRAME
  ) {
    const deltaSeconds = Math.min(remainingSeconds, MAX_SIMULATION_STEP_SECONDS);
    const shouldContinue = simulate(deltaSeconds);
    remainingSeconds = Math.max(0, remainingSeconds - deltaSeconds);
    simulatedSeconds += deltaSeconds;
    stepCount += 1;
    if (!shouldContinue) {
      break;
    }
  }

  if (remainingSeconds <= SIMULATION_TIME_EPSILON_SECONDS) {
    remainingSeconds = 0;
  }

  return {
    droppedSeconds,
    remainingSeconds,
    simulatedSeconds,
    stepCount,
  };
}
