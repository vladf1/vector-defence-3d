import { MonsterKind, type CampaignRouteData, type LevelData, type WaveData } from "./types";
import { clamp } from "./utils";

const MOBILE_WAVE_COUNT_RATIO = 0.65;
const MOBILE_SPAWN_INTERVAL_RATIO = 1.19;

function buildWaveSequence(baseSequence: MonsterKind[], levelIndex: number, waveIndex: number): MonsterKind[] {
  const source = [...new Set(baseSequence)];
  const length = clamp(4 + waveIndex + Math.floor(levelIndex / 2), 4, 12);
  const sequence: MonsterKind[] = [];
  const opening = source.includes(MonsterKind.Runner) && waveIndex % 3 === 1
    ? MonsterKind.Runner
    : (source.includes(MonsterKind.PackMan) ? MonsterKind.PackMan : source[0]);

  sequence.push(opening ?? MonsterKind.PackMan);
  for (let index = 0; index < length; index += 1) {
    sequence.push(source[(index + waveIndex + levelIndex) % source.length] ?? MonsterKind.PackMan);
  }

  const finisherOptions = [
    MonsterKind.Tank,
    MonsterKind.Splitter,
    MonsterKind.Bulwark,
    MonsterKind.Berserker,
    MonsterKind.Triangle,
    MonsterKind.Square,
  ].filter((kind) => source.includes(kind));

  if (waveIndex >= 2 && finisherOptions.length > 0) {
    const finisher = finisherOptions[(waveIndex + levelIndex) % finisherOptions.length];
    if (finisher) {
      sequence.push(finisher);
    }
  }

  return sequence;
}

function buildWave(
  levelIndex: number,
  waveIndex: number,
  waveTotal: number,
  baseSequence: MonsterKind[],
  initialBuildTime: number,
  mobile: boolean,
): WaveData {
  const countBase = 12 + (levelIndex * 1.5);
  const countStep = 3.8 + Math.floor(levelIndex / 4);
  const lastWaveBonus = waveIndex === waveTotal - 1 ? 4 + Math.round(levelIndex * 0.8) : 0;
  const countScale = mobile ? MOBILE_WAVE_COUNT_RATIO : 1;
  const count = Math.round((countBase + (waveIndex * countStep) + lastWaveBonus) * countScale);
  const pressure = (levelIndex * 0.65) + (waveIndex * 0.55);
  const spawnIntervalScale = mobile ? MOBILE_SPAWN_INTERVAL_RATIO : 1;
  const baseSpawnIntervalMin = clamp(0.78 - (pressure * 0.067), 0.23, 0.78);
  const spawnIntervalMin = baseSpawnIntervalMin * spawnIntervalScale;
  const spawnIntervalMax = clamp(baseSpawnIntervalMin + 0.32 - (Math.min(levelIndex, 6) * 0.01), baseSpawnIntervalMin + 0.11, 1.03) * spawnIntervalScale;
  const intermission = clamp(5.75 - (levelIndex * 0.18) - (waveIndex * 0.32), 2.5, 5.5);

  return {
    count,
    monsterSequence: buildWaveSequence(baseSequence, levelIndex, waveIndex),
    spawnIntervalMin,
    spawnIntervalMax,
    buildTime: waveIndex === 0 ? initialBuildTime : intermission,
    reward: Math.round((60 + (levelIndex * 9) + (waveIndex * 13)) / 10),
  };
}

export function createCampaignLevels(routes: CampaignRouteData[], mobile: boolean): LevelData[] {
  return routes.map(({ monsterSequence, ...route }, levelIndex) => {
    const waveTotal = route.waveCount ?? 6;
    const buildTime = route.initialBuildTime ?? 12;
    const waves = Array.from({ length: waveTotal }, (_, waveIndex) =>
      buildWave(levelIndex, waveIndex, waveTotal, monsterSequence, buildTime, mobile),
    );

    return {
      ...route,
      id: `campaign-${levelIndex + 1}`,
      levelNumber: levelIndex + 1,
      subtitle: route.subtitle ?? "Hold the route and keep scaling your defense.",
      waves,
      monsterCount: waves.reduce((total, wave) => total + wave.count, 0),
    };
  });
}
