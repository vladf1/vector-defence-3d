import { runBenchmarkPage } from "./benchmark-browser-harness.mjs";

const benchmarkConfig = {
  monsterCount: readNumber("MONSTER_COUNT", 720),
  towerRows: readNumber("TOWER_ROWS", 18),
  projectileCount: readNumber("PROJECTILE_COUNT", 1_200),
  missileCount: readNumber("MISSILE_COUNT", 180),
  particleCount: readNumber("PARTICLE_COUNT", 2_000),
  linkCount: readNumber("LINK_COUNT", 120),
  warmupFrames: readNumber("WARMUP_FRAMES", 18),
  measuredFrames: readNumber("MEASURED_FRAMES", 60),
};
const useCollisionIndex = process.env.COLLISION_INDEX !== "off";

const html = String.raw`
<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <title>Update Benchmarks</title>
  </head>
  <body>
    <script type="module">
      const nativeRandom = Math.random;
      let seededRandom;
      Math.random = () => seededRandom ? seededRandom() : nativeRandom();
      const { Game, createLevels } = await import("/src/game-engine.ts");
      const { DetachedBoardRenderer } = await import("/src/board-renderer.ts");
      const {
        ActiveCircleSweepCollisionIndex,
        LinearActiveCircleSweepCollisionIndex,
      } = await import("/src/game-engine/collision-detection.ts");
      const { DESKTOP_GAME_PROFILE } = await import("/src/game-profile.ts");
      const { CampaignProgressStore } = await import("/src/campaign-progress.ts");
      const { createMonster } = await import("/src/game-engine/monster-factory.ts");
      const { createPathEntriesFromDistance } = await import("/src/route-path.ts");
      const { GameState, MonsterKind, TowerKind } = await import("/src/types.ts");
      const { Particle } = await import("/src/entities/effects/particle.ts");
      const { LightningLinkEffect } = await import("/src/entities/effects/lightning-link-effect.ts");
      const { LinkEffect } = await import("/src/entities/effects/link-effect.ts");
      const { GunProjectile } = await import("/src/entities/projectiles/gun-projectile.ts");
      const { Missile } = await import("/src/entities/projectiles/missile.ts");
      const { createMissileVisual } = await import("/src/entities/projectiles/missile-visuals.ts");
      const { Drone } = await import("/src/entities/projectiles/drone.ts");
      const { UpdateResult } = await import("/src/game-engine/update-context.ts");

      const maxStressConfig = ${JSON.stringify(benchmarkConfig)};
      const useCollisionIndex = ${JSON.stringify(useCollisionIndex)};

      const profile = runMaxStressUpdateProfile(maxStressConfig);
      window.__benchmarkResults = {
        totalImpact: profile.totalImpact,
        perInvocation: profile.perInvocation,
        wholeEngine: [
          runEngineUpdateProfile("typical", { ...maxStressConfig, monsterCount: 72, towerRows: 3, projectileCount: 120, missileCount: 18, particleCount: 200, linkCount: 12 }, 12),
          runEngineUpdateProfile("stress", maxStressConfig, 60),
        ],
      };

      // Time the production engine separately; fixture maintenance is outside each sample.
      function runEngineUpdateProfile(name, config, droneCount) {
        let seed = 20260907;
        seededRandom = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296);
        try {
          const game = createBenchmarkGame();
          populateBusyBoard(game, config);
          game.runtime.spawnDelay = 0;
          game.runtime.waveSpawnedMonsters = 0;
          game.runtime.spawnedMonsters = 0;
          game.runtime.towers.push(game.createTower(TowerKind.Drone, { x: 260, y: 200 }));
          const samples = [];
          const measuredFrames = Math.max(180, config.measuredFrames);
          for (let frame = 0; frame < config.warmupFrames + measuredFrames; frame += 1) {
            maintainBusyBoard(game, config, frame);
            for (const monster of game.runtime.monsters) {
              if (monster.maxSpeedPerSecond === 0) monster.speedPerSecond = monster.maxSpeedPerSecond = 20;
            }
            game.runtime.drones.length = Math.min(game.runtime.drones.length, droneCount);
            while (game.runtime.drones.length < droneCount) {
              game.runtime.drones.push(new Drone({ x: 240, y: 180 }, game.runtime.drones.length % 7));
            }
            // Exercise deaths, splitter children, escape effects, and bounty resolution.
            if (frame % 30 === 0) {
              const monster = createMonster(MonsterKind.Splitter, game.runtime.routePath.entries, 1, 9);
              monster.hitPoints = 0;
              game.runtime.monsters.push(monster);
              const escaping = createMonster(MonsterKind.Runner, game.runtime.routePath.entries.slice(-1), 1, 9);
              game.runtime.monsters.push(escaping);
            }
            const start = performance.now();
            game.updateSimulation(1 / 60);
            const duration = performance.now() - start;
            if (frame >= config.warmupFrames) samples.push(duration);
          }
          samples.sort((a, b) => a - b);
          return {
            fixture: name,
            samples: samples.length,
            medianMs: round(samples[Math.floor(samples.length / 2)]),
            p95Ms: round(samples[Math.floor(samples.length * 0.95)]),
            meanMs: round(samples.reduce((total, value) => total + value, 0) / samples.length),
            drones: game.runtime.drones.length,
          };
        } finally {
          seededRandom = undefined;
        }
      }

      function runMaxStressUpdateProfile(config) {
        const game = createBenchmarkGame();
        game.draw = () => {
        };
        populateBusyBoard(game, config);
        const deltaSeconds = 1 / 60;

        for (let frame = 0; frame < config.warmupFrames; frame += 1) {
          profileOneRuntimeUpdate(game, deltaSeconds, undefined);
          maintainBusyBoard(game, config, frame);
        }

        const buckets = new Map();
        const frameStart = performance.now();
        for (let frame = 0; frame < config.measuredFrames; frame += 1) {
          maintainBusyBoard(game, config, frame + config.warmupFrames);
          game.setState(GameState.Playing);
          game.runtime.escapesLeft = 999999;
          profileOneRuntimeUpdate(game, deltaSeconds, buckets);
          boundBusyBoard(game, config);
        }
        const elapsedWithFixtureMaintenance = performance.now() - frameStart;
        const timedUpdateMs = Array.from(buckets.values())
          .reduce((sum, bucket) => sum + bucket.totalMs, 0);

        const rows = Array.from(buckets.entries())
          .map(([method, bucket]) => ({
            method,
            avgInstances: round(bucket.calls / config.measuredFrames),
            totalMs: round(bucket.totalMs),
            msPerFrame: round(bucket.totalMs / config.measuredFrames),
            usPerCall: round((bucket.totalMs * 1000) / Math.max(1, bucket.calls)),
            finalInstances: bucket.lastInstances,
          }))
          .concat([{
            method: "TOTAL:timed-update-methods",
            avgInstances: "",
            totalMs: round(timedUpdateMs),
            msPerFrame: round(timedUpdateMs / config.measuredFrames),
            usPerCall: "",
            finalInstances: [
              "monsters=" + game.runtime.monsters.length,
              "towers=" + game.runtime.towers.length,
              "projectiles=" + game.runtime.projectiles.length,
              "missiles=" + game.runtime.missiles.length,
              "particles=" + game.runtime.particles.length,
              "links=" + game.runtime.links.length,
            ].join(" "),
          }, {
            method: "benchmark:fixture-maintenance",
            avgInstances: "",
            totalMs: round(elapsedWithFixtureMaintenance - timedUpdateMs),
            msPerFrame: round((elapsedWithFixtureMaintenance - timedUpdateMs) / config.measuredFrames),
            usPerCall: "",
            finalInstances: "",
          }])
        return {
          totalImpact: rows.toSorted((a, b) => {
            if (a.method.startsWith("TOTAL:")) {
              return 1;
            }
            if (b.method.startsWith("TOTAL:")) {
              return -1;
            }
            return b.msPerFrame - a.msPerFrame;
          }),
          perInvocation: rows
            .filter((row) => typeof row.usPerCall === "number")
            .toSorted((a, b) => b.usPerCall - a.usPerCall),
        };
      }

      function profileOneRuntimeUpdate(game, deltaSeconds, buckets) {
        let activeMonsters = game.runtime.monsters.filter((monster) => !monster.removed && monster.hitPoints > 0);
        let updateContext = createUpdateContext(game, deltaSeconds, activeMonsters);
        const updateResult = new UpdateResult();
        timeGroupedByConstructor(buckets, "monster", game.runtime.monsters, (monster) => {
          monster.update(updateContext, updateResult);
        });
        activeMonsters = game.runtime.monsters.filter((monster) => !monster.removed && monster.hitPoints > 0);
        if (useCollisionIndex) {
          timeGroup(buckets, "collision:ActiveCircleSweepCollisionIndex.rebuild", [game.benchmarkMonsterCollisionIndex], (index) => {
            index.rebuild(activeMonsters);
          });
        }
        updateContext = createUpdateContext(game, deltaSeconds, activeMonsters);
        timeGroup(buckets, "projectile:Projectile.update", game.runtime.projectiles, (projectile) => {
          projectile.update(updateContext, updateResult);
        });
        timeGroup(buckets, "projectile:Missile.update", game.runtime.missiles, (missile) => {
          missile.update(updateContext, updateResult);
        });
        timeGroupedByConstructor(buckets, "particle", game.runtime.particles, (particle) => {
          particle.update(updateContext);
        });
        timeGroupedByConstructor(buckets, "link", game.runtime.links, (link) => {
          link.update(updateContext);
        });
        timeGroupedByConstructor(buckets, "tower", game.runtime.towers, (tower) => {
          tower.update(updateContext, updateResult);
        });
        timeGroup(buckets, "runtime:compactRemoved", [game.runtime], (runtime) => {
          runtime.compactRemoved();
        });
      }

      function createUpdateContext(game, deltaSeconds, activeMonsters) {
        return {
          deltaSeconds,
          fieldWidth: game.profile.fieldWidth,
          fieldHeight: game.profile.fieldHeight,
          fieldBounds: { minX: 0, minY: 0, maxX: game.profile.fieldWidth, maxY: game.profile.fieldHeight },
          activeMonsters,
          monsterCollisionIndex: useCollisionIndex
            ? game.benchmarkMonsterCollisionIndex
            : new LinearActiveCircleSweepCollisionIndex(activeMonsters),
          activeDrones: game.runtime.drones,
          droneAssignments: new Map(),
        };
      }

      function timeGroupedByConstructor(buckets, category, items, update) {
        const groups = groupByConstructor(items);
        for (const [constructorName, group] of groups) {
          timeGroup(buckets, category + ":" + constructorName + ".update", group, update);
        }
      }

      function groupByConstructor(items) {
        const groups = new Map();
        for (const item of items) {
          const name = item.constructor?.name ?? "Unknown";
          const group = groups.get(name);
          if (group) {
            group.push(item);
          } else {
            groups.set(name, [item]);
          }
        }
        return groups;
      }

      function timeGroup(buckets, method, items, update) {
        const start = performance.now();
        for (const item of items) {
          update(item);
        }
        const elapsed = performance.now() - start;
        if (!buckets) {
          return;
        }
        const bucket = buckets.get(method) ?? { totalMs: 0, calls: 0, lastInstances: 0 };
        bucket.totalMs += elapsed;
        bucket.calls += items.length;
        bucket.lastInstances = items.length;
        buckets.set(method, bucket);
      }

      function createBenchmarkGame() {
        const audio = { play() {} };
        const game = new Game(
          createLevels("desktop"),
          audio,
          DESKTOP_GAME_PROFILE,
          new CampaignProgressStore(undefined),
        );
        // Simulation-only fixture: visible bounds are exactly the field.
        game.setRenderer(new DetachedBoardRenderer({
          minX: 0,
          minY: 0,
          maxX: DESKTOP_GAME_PROFILE.fieldWidth,
          maxY: DESKTOP_GAME_PROFILE.fieldHeight,
        }));
        game.benchmarkMonsterCollisionIndex = new ActiveCircleSweepCollisionIndex(64);
        game.startLevel(game.levels[9]);
        game.runtime.spawnDelay = 999;
        game.runtime.waveSpawnedMonsters = game.activeWave?.count ?? 999;
        game.runtime.spawnedMonsters = game.currentLevel?.monsterCount ?? 999;
        game.runtime.escapesLeft = 999999;
        game.bannerTimer = 0;
        game.setState(GameState.Playing);
        return game;
      }

      function populateBusyBoard(game, config) {
        const routePath = game.runtime.routePath;
        if (!routePath) {
          throw new Error("Expected benchmark level to have a route path.");
        }
        const routeLength = routePath.entries[routePath.entries.length - 1].totalDistance;
        const monsterKinds = [
          MonsterKind.PackMan,
          MonsterKind.Square,
          MonsterKind.Triangle,
          MonsterKind.Tank,
          MonsterKind.Runner,
          MonsterKind.Splitter,
          MonsterKind.Berserker,
          MonsterKind.Bulwark,
        ];
        for (let index = 0; index < config.monsterCount; index += 1) {
          const distance = 12 + ((routeLength - 36) * ((index % config.monsterCount) / config.monsterCount));
          const monster = createMonster(
            monsterKinds[index % monsterKinds.length],
            createPathEntriesFromDistance(routePath.entries, distance),
            game.profile.monsterSpeedScale,
            game.currentLevelIndex,
          );
          makeBenchmarkMonsterDurable(monster);
          game.runtime.monsters.push(monster);
        }

        const towerKinds = [TowerKind.Gun, TowerKind.Laser, TowerKind.Missile, TowerKind.Slow, TowerKind.Lightning];
        for (let row = 0; row < config.towerRows; row += 1) {
          for (let column = 0; column < towerKinds.length; column += 1) {
            const tower = game.createTower(towerKinds[column], {
              x: 80 + (column * 92) + ((row % 2) * 22),
              y: 70 + (row * 54),
            });
            while (tower.canUpgrade()) {
              tower.upgrade();
            }
            tower.cooldownSeconds = 0;
            tower.angle = 0;
            game.runtime.towers.push(tower);
          }
        }

        maintainBusyBoard(game, config, 0);
      }

      function maintainBusyBoard(game, config, frame) {
        boundBusyBoard(game, config);
        refillMonsters(game, config.monsterCount);
        refillProjectiles(game, config.projectileCount);
        refillMissiles(game, config.missileCount);
        refillParticles(game, config.particleCount, frame);
        refillLinks(game, config.linkCount);
      }

      function boundBusyBoard(game, config) {
        game.runtime.monsters.length = Math.min(game.runtime.monsters.length, config.monsterCount);
        game.runtime.projectiles.length = Math.min(game.runtime.projectiles.length, config.projectileCount);
        game.runtime.missiles.length = Math.min(game.runtime.missiles.length, config.missileCount);
        game.runtime.particles.length = Math.min(game.runtime.particles.length, config.particleCount * 3);
        game.runtime.links.length = Math.min(game.runtime.links.length, config.linkCount * 3);
      }

      function refillMonsters(game, count) {
        const routePath = game.runtime.routePath;
        if (!routePath) {
          return;
        }
        const routeLength = routePath.entries[routePath.entries.length - 1].totalDistance;
        const kinds = [MonsterKind.Runner, MonsterKind.Tank, MonsterKind.Bulwark, MonsterKind.Berserker, MonsterKind.Splitter];
        while (game.runtime.monsters.length < count) {
          const index = game.runtime.monsters.length;
          const distance = 18 + ((routeLength - 48) * ((index % count) / count));
          const monster = createMonster(
            kinds[index % kinds.length],
            createPathEntriesFromDistance(routePath.entries, distance),
            game.profile.monsterSpeedScale,
            game.currentLevelIndex,
          );
          makeBenchmarkMonsterDurable(monster);
          game.runtime.monsters.push(monster);
        }
      }

      function makeBenchmarkMonsterDurable(monster) {
        monster.hitPoints = 1_000_000_000_000;
        monster.maxHitPoints = 1_000_000_000_000;
        monster.speedPerSecond = 0;
        monster.maxSpeedPerSecond = 0;
        monster.velocityXPerSecond = 0;
        monster.velocityYPerSecond = 0;
      }

      function refillProjectiles(game, count) {
        const targets = game.runtime.monsters;
        while (game.runtime.projectiles.length < count && targets.length > 0) {
          const index = game.runtime.projectiles.length;
          const targetMonster = targets[index % targets.length];
          const projectile = new GunProjectile(
            { x: 40 + ((index * 37) % 500), y: 40 + ((index * 53) % 300) },
            { x: targetMonster.x + ((index % 5) * 6), y: targetMonster.y },
            0,
          );
          projectile.x = 4 + (index % 7);
          projectile.y = 4 + (index % 11);
          projectile.velocityXPerSecond = 0;
          projectile.velocityYPerSecond = 0;
          game.runtime.projectiles.push(projectile);
        }
      }

      function refillMissiles(game, count) {
        const targets = game.runtime.monsters;
        while (game.runtime.missiles.length < count && targets.length > 0) {
          const index = game.runtime.missiles.length;
          const targetMonster = targets[(index * 3) % targets.length];
          const missile = new Missile(
            { x: 60 + ((index * 71) % 460), y: 60 + ((index * 41) % 280) },
            targetMonster,
            0,
            createMissileVisual(0),
            0.3 + (index * 0.17),
          );
          missile.x = 260 + (index % 17);
          missile.y = 20 + (index % 13);
          missile.trackedMonster = undefined;
          missile.angle = 0;
          missile.speedPerSecond = -180;
          game.runtime.missiles.push(missile);
        }
      }

      function refillParticles(game, count, frame) {
        while (game.runtime.particles.length < count) {
          const index = game.runtime.particles.length + frame;
          game.runtime.particles.push(new Particle(
            30 + ((index * 29) % 560),
            30 + ((index * 31) % 340),
            2 + (index % 4),
            index % 2 === 0 ? "#8ff7ff" : "#ff8f45",
            0.15,
            { speedPerSecond: 30 + (index % 80), offset: 0, angle: index * 0.31 },
          ));
        }
      }

      function refillLinks(game, count) {
        const targets = game.runtime.monsters;
        while (game.runtime.links.length < count && targets.length > 0) {
          const index = game.runtime.links.length;
          const targetMonster = targets[(index * 5) % targets.length];
          const source = {
            x: 90 + ((index * 83) % 420),
            y: 70 + ((index * 47) % 260),
            level: 6,
            removed: false,
          };
          game.runtime.links.push(index % 2 === 0
            ? new LightningLinkEffect(source, targetMonster, "#8ff7ff")
            : new LinkEffect(targetMonster, "#d8ff4f", 0.2, source));
        }
      }

      function round(value) {
        return Math.round(value * 1000) / 1000;
      }
    </script>
  </body>
</html>
`;

const value = await runBenchmarkPage({
  html,
  path: "/__update-benchmark",
  pluginName: "update-benchmark-page",
  timeoutMs: 120_000,
});

console.log("Whole engine update (no drawing; fixture maintenance excluded)");
console.table(value.wholeEngine);
console.log("Reconstructed update slice: total method impact");
console.table(value.totalImpact);
console.log("Per update invocation");
console.table(value.perInvocation);

function readNumber(name, fallback) {
  const value = Number(process.env[name] ?? fallback);
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`Invalid ${name}: ${process.env[name]}`);
  }
  return value;
}
