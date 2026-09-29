import { runBenchmarkPage } from "./benchmark-browser-harness.mjs";

const benchmarkConfig = {
  warmupSamples: readPositiveInteger("WARMUP_SAMPLES", 40),
  minimumWarmupDeaths: readPositiveInteger("MINIMUM_WARMUP_DEATHS", 2_000),
  measuredSamples: readPositiveInteger("MEASURED_SAMPLES", 500),
  massDeathCounts: readPositiveIntegerList("MASS_DEATH_COUNTS", [8, 24, 48]),
  throughputDeaths: readPositiveInteger("THROUGHPUT_DEATHS", 1_000),
  throughputTrials: readPositiveInteger("THROUGHPUT_TRIALS", 15),
};

const html = String.raw`
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>Death Effect Construction Benchmarks</title>
  </head>
  <body>
    <script type="module">
      let randomState = 0;
      Math.random = benchmarkRandom;

      const { BerserkerMonster } = await import("/src/entities/monsters/berserker-monster.ts");
      const { BulwarkMonster } = await import("/src/entities/monsters/bulwark-monster.ts");
      const { PackManMonster } = await import("/src/entities/monsters/packman-monster.ts");
      const { PolygonShardSplitter } = await import("/src/entities/monsters/polygon-shard-splitter.ts");
      const { RunnerMonster } = await import("/src/entities/monsters/runner-monster.ts");
      const { SplitterMonster } = await import("/src/entities/monsters/splitter-monster.ts");
      const { SquareMonster } = await import("/src/entities/monsters/square-monster.ts");
      const { TankMonster } = await import("/src/entities/monsters/tank-monster.ts");
      const { TriangleMonster } = await import("/src/entities/monsters/triangle-monster.ts");
      const { UpdateResult } = await import("/src/game-engine/update-context.ts");

      const config = ${JSON.stringify(benchmarkConfig)};
      const path = [
        { x: 480, y: 270, totalDistance: 0 },
        { x: 800, y: 270, totalDistance: 320 },
      ];
      const monsterSpecs = [
        ["berserker", () => new BerserkerMonster(path, 1)],
        ["bulwark", () => new BulwarkMonster(path, 1)],
        ["packman", () => new PackManMonster(path, 1)],
        ["runner", () => new RunnerMonster(path, 1)],
        ["splitter", () => new SplitterMonster(path, 1)],
        ["square", () => new SquareMonster(path, 1)],
        ["tank", () => new TankMonster(path, 1)],
        ["triangle", () => new TriangleMonster(path, 1)],
      ];
      const monsters = monsterSpecs.map(([, createMonster]) => createMonster());
      const originalSplitIntoShards = PolygonShardSplitter.prototype.splitIntoShards;
      let activeSplitMetrics;

      PolygonShardSplitter.prototype.splitIntoShards = function (outline) {
        const start = performance.now();
        const shards = originalSplitIntoShards.call(this, outline);
        if (activeSplitMetrics) {
          activeSplitMetrics.durationMs += performance.now() - start;
          activeSplitMetrics.calls += 1;
          activeSplitMetrics.shards += shards.length;
        }
        return shards;
      };

      const perMonster = [];
      for (let monsterIndex = 0; monsterIndex < monsters.length; monsterIndex += 1) {
        const [name] = monsterSpecs[monsterIndex];
        const monster = monsters[monsterIndex];
        const warmupCount = getWarmupSampleCount(1);
        for (let sample = 0; sample < warmupCount; sample += 1) {
          constructDeathEffects([monster], sample, 1, monsterIndex + 1);
        }

        const measurements = [];
        for (let sample = 0; sample < config.measuredSamples; sample += 1) {
          measurements.push(constructDeathEffects([monster], sample, 1, monsterIndex + 1));
          if ((sample + 1) % 50 === 0) {
            await nextAnimationFrame();
          }
        }
        perMonster.push(summarizeScenario(name, 1, measurements));
      }

      const massDeaths = [];
      for (const deathCount of config.massDeathCounts) {
        const warmupCount = getWarmupSampleCount(deathCount);
        for (let sample = 0; sample < warmupCount; sample += 1) {
          constructDeathEffects(monsters, sample, deathCount);
        }

        const measurements = [];
        for (let sample = 0; sample < config.measuredSamples; sample += 1) {
          await nextAnimationFrame();
          measurements.push(constructDeathEffects(monsters, sample, deathCount));
        }
        massDeaths.push(summarizeScenario("balanced-roster", deathCount, measurements));
      }

      const throughputMeasurements = [];
      const throughputWarmupCount = getWarmupSampleCount(config.throughputDeaths);
      for (let sample = 0; sample < throughputWarmupCount; sample += 1) {
        constructDeathEffects(monsters, sample, config.throughputDeaths);
      }
      for (let trial = 0; trial < config.throughputTrials; trial += 1) {
        await nextAnimationFrame();
        throughputMeasurements.push(
          constructDeathEffects(monsters, trial, config.throughputDeaths),
        );
      }

      window.__benchmarkResults = {
        config,
        perMonster,
        massDeaths,
        throughput: summarizeScenario(
          "balanced-roster-throughput",
          config.throughputDeaths,
          throughputMeasurements,
        ),
      };

      function constructDeathEffects(monsterPool, sample, count = monsterPool.length, seedOffset = 0) {
        resetBenchmarkRandom(sample, count, seedOffset);
        const result = new UpdateResult();
        activeSplitMetrics = { durationMs: 0, calls: 0, shards: 0 };
        const start = performance.now();
        for (let index = 0; index < count; index += 1) {
          monsterPool[(sample + index) % monsterPool.length].addDeathEffect(result);
        }
        const durationMs = performance.now() - start;
        const splitMetrics = activeSplitMetrics;
        activeSplitMetrics = undefined;
        return {
          durationMs,
          splitterMs: splitMetrics.durationMs,
          shardCount: splitMetrics.shards,
          particleCount: result.particles.length,
        };
      }

      function getWarmupSampleCount(deathCount) {
        return Math.max(
          config.warmupSamples,
          Math.ceil(config.minimumWarmupDeaths / deathCount),
        );
      }

      function summarizeScenario(name, deathCount, measurements) {
        const durations = measurements.map((measurement) => measurement.durationMs);
        const totalDurationMs = sum(durations);
        const totalSplitterMs = sum(measurements.map((measurement) => measurement.splitterMs));
        return {
          scenario: name,
          deathsPerFrame: deathCount,
          samples: measurements.length,
          p50Ms: percentile(durations, 0.5),
          p95Ms: percentile(durations, 0.95),
          p99Ms: percentile(durations, 0.99),
          maxMs: round(Math.max(...durations)),
          meanMs: round(totalDurationMs / measurements.length),
          meanUsPerDeath: round((totalDurationMs * 1000) / (measurements.length * deathCount)),
          splitterSharePct: round((totalSplitterMs / totalDurationMs) * 100),
          meanShardsPerDeath: round(
            sum(measurements.map((measurement) => measurement.shardCount))
              / (measurements.length * deathCount),
          ),
          meanParticlesPerDeath: round(
            sum(measurements.map((measurement) => measurement.particleCount))
              / (measurements.length * deathCount),
          ),
        };
      }

      function percentile(values, ratio) {
        const sorted = values.toSorted((left, right) => left - right);
        const index = Math.max(0, Math.ceil(sorted.length * ratio) - 1);
        return round(sorted[index]);
      }

      function sum(values) {
        return values.reduce((total, value) => total + value, 0);
      }

      function round(value) {
        return Math.round(value * 1000) / 1000;
      }

      function nextAnimationFrame() {
        return new Promise((resolve) => requestAnimationFrame(resolve));
      }

      function resetBenchmarkRandom(sample, deathCount, seedOffset) {
        randomState = (
          0x9E3779B9
          ^ Math.imul(sample + 1, 0x85EBCA6B)
          ^ Math.imul(deathCount + 1, 0xC2B2AE35)
          ^ Math.imul(seedOffset + 1, 0x27D4EB2F)
        ) >>> 0;
      }

      function benchmarkRandom() {
        randomState = (randomState + 0x6D2B79F5) >>> 0;
        let value = randomState;
        value = Math.imul(value ^ (value >>> 15), value | 1);
        value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
        return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
      }
    </script>
  </body>
</html>
`;

const value = await runBenchmarkPage({
  html,
  path: "/__death-effect-benchmark",
  pluginName: "death-effect-benchmark-page",
  timeoutMs: 120_000,
});

console.log("Configuration");
console.table(value.config);
console.log("Per-monster construction");
console.table(value.perMonster);
console.log("Mass-death frame construction");
console.table(value.massDeaths);
console.log("Sustained deterministic throughput");
console.table([value.throughput]);

function readPositiveInteger(name, fallback) {
  const value = Number(process.env[name] ?? fallback);
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`Invalid ${name}: ${process.env[name]}`);
  }
  return value;
}

function readPositiveIntegerList(name, fallback) {
  if (process.env[name] === undefined) {
    return fallback;
  }

  const values = process.env[name].split(",").map((value) => Number(value.trim()));
  if (values.length === 0 || values.some((value) => !Number.isInteger(value) || value <= 0)) {
    throw new Error(`Invalid ${name}: ${process.env[name]}`);
  }
  return values;
}
