import { WEBGPU_LAUNCH_ARGS, runBrowserPage } from "./benchmark-browser-harness.mjs";

const checks = await runBrowserPage({
  path: "/__runtime-checks",
  html: "<!doctype html><body></body>",
  pluginName: "runtime-checks",
  launchArgs: WEBGPU_LAUNCH_ARGS,
}, (page) => page.evaluate(async () => {
  let seed = 42;
  Math.random = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 4294967296);
  const { Game, createLevels } = await import("/src/game-engine.ts");
  const { createMonster } = await import("/src/game-engine/monster-factory.ts");
  const { UpdateResult } = await import("/src/game-engine/update-context.ts");
  const { ActiveCircleSweepCollisionIndex, LinearActiveCircleSweepCollisionIndex } = await import("/src/game-engine/collision-detection.ts");
  const { GunTower } = await import("/src/entities/towers/gun-tower.ts");
  const { LaserTower } = await import("/src/entities/towers/laser-tower.ts");
  const { SlowTower } = await import("/src/entities/towers/slow-tower.ts");
  const { LightningTower } = await import("/src/entities/towers/lightning-tower.ts");
  const { Drone } = await import("/src/entities/projectiles/drone.ts");
  const { GunProjectile } = await import("/src/entities/projectiles/gun-projectile.ts");
  const { DESKTOP_GAME_PROFILE, MOBILE_GAME_PROFILE } = await import("/src/game-profile.ts");
  const { CampaignProgressStore } = await import("/src/campaign-progress.ts");
  const { calculateIntercept, isWithinDistanceToSegment } = await import("/src/utils.ts");
  const { createHudSnapshot, createModalView } = await import("/src/game-view.ts");
  const { createGameSession } = await import("/src/game-session.ts");
  const { createBoardCameraRig } = await import("/src/render3d/camera-rig.ts");
  const { GameAudio } = await import("/src/game-audio.ts");
  const { default: authoredLevels } = await import("/game-levels.json?import");
  const { runBoundedSimulationSubsteps } = await import("/src/simulation-timing.ts");
  const { Missile } = await import("/src/entities/projectiles/missile.ts");
  const { createMissileVisual } = await import("/src/entities/projectiles/missile-visuals.ts");
  const { createPolygonShardParticles } = await import("/src/entities/monsters/death-effect-helpers.ts");
  const { createHitImpactParticles, createLaserImpactParticles, createMissileExplosionParticles, createEscapeBurstParticles, ESCAPE_BURST_CONFIG } = await import("/src/game-engine/combat-effects.ts");
  const passed = [];
  const check = (condition, name) => {
    if (!condition) throw new Error(name);
    passed.push(name);
  };
  const near = (a, b) => Math.abs(a - b) < 1e-7;
  const path = [{ x: 100, y: 100, totalDistance: 0 }, { x: 1000, y: 100, totalDistance: 900 }];
  const result = new UpdateResult();
  const target = createMonster("square", path, 1, 0);
  target.velocityXPerSecond = target.velocityYPerSecond = 0;
  const index = new ActiveCircleSweepCollisionIndex(64);
  index.rebuild([target]);
  const context = { deltaSeconds: 1 / 60, fieldWidth: 1200, fieldHeight: 600, fieldBounds: { minX: 0, minY: 0, maxX: 1200, maxY: 600 }, activeMonsters: [target], activeDrones: [], droneAssignments: new Map(), monsterCollisionIndex: index };
  // Headless stand-in for the WebGPU board: the real camera rig supplies visible bounds and picking.
  class CameraBoundsRenderer {
    constructor(profile) {
      this.canvas = document.createElement("canvas");
      this.rig = createBoardCameraRig(profile.fieldWidth, profile.fieldHeight);
    }
    resize() {
      const rect = this.canvas.getBoundingClientRect();
      this.rig.resize(Math.max(1, rect.width), Math.max(1, rect.height));
    }
    renderBackgroundLayer() {}
    draw() {}
    getVisibleFieldBounds() { return this.rig.fieldBounds; }
    tiltBy(deltaRadians) { return this.rig.tiltBy(deltaRadians); }
    zoomAt(factor, clientX, clientY, rect) { return this.rig.zoomAt(factor, clientX, clientY, rect); }
    panBetween(fromX, fromY, toX, toY, rect) { return this.rig.panBetween(fromX, fromY, toX, toY, rect); }
    resetView() { return this.rig.resetView(); }
    isPointInUpgradeButton() { return false; }
    isPointInLaserLockButton() { return false; }
    clientToField(clientX, clientY, rect) { return this.rig.clientToField(clientX, clientY, rect); }
    dispose() {}
  }
  function makeGame(profile) {
    const game = new Game(createLevels(profile.mode), { play() {} }, profile, new CampaignProgressStore(undefined));
    game.setRenderer(new CameraBoundsRenderer(profile));
    return game;
  }

  function countShots(entity, seconds, hz) {
    context.deltaSeconds = 1 / hz;
    let shots = 0;
    for (let frame = 0; frame < hz * seconds; frame++) {
      result.clear();
      entity.update(context, result);
      shots += result.projectiles.length;
    }
    return shots;
  }

  for (const hz of [60, 120, 144, 240]) {
    const gun = new GunTower(0, 100);
    gun.angle = 0;
    gun.range = 200;
    const shots = countShots(gun, 60, hz);
    check(shots === 300, `Gun: ${shots} shots in 60 seconds at ${hz} Hz`);
    const drone = new Drone({ x: 60, y: 100 }, 6);
    const droneShots = countShots(drone, 10, hz);
    check(droneShots === 27, `Drone: ${droneShots} shots in 10 seconds at ${hz} Hz`);
    const game = makeGame(DESKTOP_GAME_PROFILE);
    game.startLevelByIndex(0);
    game.runtime.spawnDelay = 0;
    Object.assign(game.activeWave, { count: 1000, spawnIntervalMin: 0.23, spawnIntervalMax: 0.23 });
    game.runtime.escapesLeft = 1000;
    for (let frame = 0; frame < hz * 10; frame++) game.updateSimulation(1 / hz);
    check(game.runtime.spawnedMonsters === 43, `Spawning: ${game.runtime.spawnedMonsters} monsters in 10 seconds at ${hz} Hz`);
  }
  context.deltaSeconds = 1 / 60;
  const idleGun = new GunTower(0, 100);
  idleGun.angle = 0;
  idleGun.range = 200;
  for (let frame = 0; frame < 600; frame++) idleGun.update({ ...context, activeMonsters: [] }, result);
  const idleShots = countShots(idleGun, 0.2, 60);
  check(idleShots === 1, "Idle weapons do not bank shots");

  const sweep = { previousX: 50, previousY: 100, x: 150, y: 100, radius: 1 };
  const collisionTime = index.findEarliestCollision(sweep).time;
  target.shakeFromHit();
  check(target.x === 100 && target.y === 100 && index.findEarliestCollision(sweep).time === collisionTime, "Hit shake leaves position and swept collision unchanged");
  for (const kind of ["packman", "square", "triangle", "tank", "runner", "splitter", "berserker", "bulwark"]) {
    const monster = createMonster(kind, path, 1, 0);
    seed = 42;
    const before = new UpdateResult();
    monster.addDeathEffect(before);
    monster.shakeFromHit();
    const dx = monster.visualX - monster.x, dy = monster.visualY - monster.y;
    seed = 42;
    const after = new UpdateResult();
    monster.addDeathEffect(after);
    const matchingOffsets = before.particles.length === after.particles.length
      && before.particles.every((particle, index) =>
        near(after.particles[index].x - particle.x, dx) && near(after.particles[index].y - particle.y, dy));
    check(matchingOffsets, `${kind}: death particles match the visual shake offset`);
  }
  for (const [velocity, expected] of [[10, 125], [-50, 50], [-100, 100 / 3], [100, 100]]) {
    const intercept = calculateIntercept({ x: 100, y: 0, velocityXPerSecond: velocity, velocityYPerSecond: 0 }, 50, { x: 0, y: 0 });
    check(near(intercept.x, expected), `Intercept: target velocity ${velocity}`);
  }
  const laser = new LaserTower(0, 100);
  laser.angle = 0;
  laser.range = 200;
  const endpoint = laser.beamTarget;
  laser.update(context, result);
  laser.directionLocked = true;
  laser.update(context, result);
  check(laser.beamTarget === endpoint && near(endpoint.x, 1000) && near(endpoint.y, 100), "Laser reuses its endpoint in tracked and locked modes");

  const authoredSequence = authoredLevels[0].monsterSequence;
  try {
    for (const sequence of [["typo-monster"], []]) {
      authoredLevels[0].monsterSequence = sequence;
      let message = "";
      try { createLevels("desktop"); } catch (error) { message = error.message; }
      check(message.includes(authoredLevels[0].name) && message.includes(sequence[0] ?? "non-empty"), `Malformed monster sequence ${JSON.stringify(sequence)} reports its level and cause: ${message}`);
    }
  } finally { authoredLevels[0].monsterSequence = authoredSequence; }
  const authoredTowers = authoredLevels[0].availableTowers;
  try {
    authoredLevels[0].availableTowers = [...authoredTowers, "lightning"];
    let message = "";
    try { createLevels("desktop"); } catch (error) { message = error.message; }
    check(message.includes(authoredLevels[0].name) && message.includes('shortcut "5"'), "Level data rejects conflicting tower shortcuts");
  } finally { authoredLevels[0].availableTowers = authoredTowers; }
  for (const profile of [DESKTOP_GAME_PROFILE, MOBILE_GAME_PROFILE]) {
    const game = makeGame(profile);
    game.startLevelByIndex(0);
    const tower = game.createTower("gun", { x: 50, y: 100 });
    game.runtime.selectedTower = tower;
    const hud = createHudSnapshot(game);
    check(hud.selectionName === (profile.mode === "mobile" ? "Gun Tower" : "Gun Tower · Level 1 · Range 60") && hud.upgradeLabel === "Upgrade - $5", `${profile.mode}: selection and action labels`);
    game.finishLevel();
    check(createModalView(game).starAward.title === "Perfect route", `${profile.mode}: star award copy`);
    game.startLevelByIndex(0);
    game.runtime.spawnDelay = 999;
    const monster = createMonster("square", path, 1, 0);
    game.runtime.monsters.push(monster);
    game.updateSimulation(1 / 60);
    // No projectiles: the collision index can stay dormant.
    const shot = new GunProjectile({ x: monster.x, y: monster.y }, { x: monster.x + 1, y: monster.y }, 0);
    game.runtime.projectiles.push(shot);
    game.updateSimulation(1 / 60);
    check(shot.removed && monster.hitPoints < monster.maxHitPoints, `${profile.mode}: first projectile activates collision queries`);
  }

  for (const seconds of [0, -1, 1 / 240, 1 / 60, 0.1, 2]) {
    const steps = [];
    const timing = runBoundedSimulationSubsteps(seconds, delta => { steps.push(delta); return true; });
    check(steps.length <= 8 && steps.every(delta => delta <= 1 / 60) && near(timing.simulatedSeconds + timing.remainingSeconds + timing.droppedSeconds, Math.max(0, seconds)), `Bounded steps conserve time for ${seconds}s`);
  }
  const stopped = runBoundedSimulationSubsteps(0.1, () => false);
  check(stopped.stepCount === 1 && near(stopped.remainingSeconds, 0.1 - 1 / 60), "Substeps stop immediately when the simulation stops");

  const random = (min, max) => min + Math.random() * (max - min);
  const sweeps = Array.from({ length: 100 }, (_, i) => ({ x: random(-200, 900), y: random(-100, 600), previousX: random(-200, 900), previousY: random(-100, 600), radius: random(1, 30), removed: i % 11 === 0, hitPoints: i % 13 === 0 ? 0 : 100 }));
  const linear = new LinearActiveCircleSweepCollisionIndex(sweeps);
  index.rebuild(sweeps);
  for (let i = 0; i < 200; i++) {
    const source = { x: random(-200, 900), y: random(-100, 600), previousX: random(-200, 900), previousY: random(-100, 600), radius: 2 };
    const indexed = index.findEarliestCollision(source), expected = linear.findEarliestCollision(source);
    if (indexed?.target !== expected?.target || (indexed && !near(indexed.time, expected.time))) throw new Error(`Collision index disagrees with linear query ${i}`);
  }
  passed.push("200 seeded swept-collision queries match the linear implementation");

  const beam = new LaserTower(0, 100);
  for (let i = 0; i < 200; i++) {
    beam.angle = random(-Math.PI, Math.PI);
    beam.directionLocked = true;
    beam.beamAlpha = 1; beam.cooldownSeconds = 10;
    const sample = createMonster("square", path, 0, 0);
    sample.x = random(-1100, 1100); sample.y = random(-1000, 1200); sample.radius = random(1, 100);
    beam.update({ ...context, deltaSeconds: 1 / 60, activeMonsters: [sample] }, result);
    const source = { x: beam.x + Math.cos(beam.angle) * 8.5, y: beam.y + Math.sin(beam.angle) * 8.5 };
    const expected = isWithinDistanceToSegment(sample, source, beam.beamTarget, sample.radius);
    if ((sample.hitPoints < sample.maxHitPoints) !== expected) throw new Error(`Beam geometry disagrees with segment query ${i}`);
  }
  passed.push("200 seeded beam intersections match segment geometry");
  for (const hz of [60, 120, 144, 240]) {
    const victim = createMonster("bulwark", path, 1, 0), laser = new LaserTower(0, 100);
    laser.angle = 0; laser.directionLocked = true; laser.beamAlpha = 1; laser.cooldownSeconds = 10;
    for (let i = 0; i < hz; i++) laser.update({ ...context, deltaSeconds: 1 / hz, activeMonsters: [victim] }, result);
    check(near(victim.maxHitPoints - victim.hitPoints, 33), `Laser fade integrates to 33 damage through armor at ${hz} Hz`);
  }

  for (const [profile, width, height, source, destination] of [
    [MOBILE_GAME_PROFILE, 390, 800, { x: 195, y: 600 }, { x: 195, y: 540 }],
    [DESKTOP_GAME_PROFILE, 1000, 450, { x: -40, y: 225 }, { x: 40, y: 225 }],
  ]) {
    const game = makeGame(profile), canvas = game.renderer.canvas;
    canvas.style.cssText = `width:${width}px;height:${height}px`;
    document.body.append(canvas); game.resize(); game.startLevelByIndex(0); game.runtime.spawnDelay = 999;
    check(game.canPlaceTower(source), `${profile.mode}: expanded board permits the test placement`);
    const victim = createMonster("square", [{ ...destination, totalDistance: 0 }, { x: 900, y: destination.y, totalDistance: 900 - destination.x }], 0, 0);
    game.runtime.monsters.push(victim);
    const shot = new GunProjectile(source, destination, 0), missile = new Missile(source, victim, 0, createMissileVisual(0));
    game.runtime.projectiles.push(shot); game.runtime.missiles.push(missile);
    game.updateSimulation(1 / 60);
    check(!shot.removed && !missile.removed, `${profile.mode}: shots survive within expanded board bounds`);
    for (let i = 0; i < 30; i++) game.updateSimulation(1 / 60);
    check(shot.removed && victim.hitPoints < victim.maxHitPoints, `${profile.mode}: expanded-board shot reaches and damages its target`);
    game.openMenu(); game.openMenu(); game.resumeBattle();
    check(game.state === "playing", `${profile.mode}: repeated map opening preserves resume state`);
    game.togglePause(); game.openMenu(); game.openMenu(); game.resumeBattle();
    check(game.state === "paused", `${profile.mode}: repeated map opening preserves paused state`);
    canvas.remove();
  }

  {
    const { fieldWidth, fieldHeight } = DESKTOP_GAME_PROFILE;
    const rig = createBoardCameraRig(fieldWidth, fieldHeight);
    rig.resize(800, 450);
    const rect = new DOMRect(0, 0, 800, 450);
    const initialBounds = { ...rig.fieldBounds };
    const initialTilt = rig.tilt;
    const corners = [[0, 0], [fieldWidth, 0], [0, fieldHeight], [fieldWidth, fieldHeight]];
    const framesField = () => corners.every(([x, y]) => {
      const screen = rig.projectToViewport(x, 0, y, { x: 0, y: 0 });
      return screen.x >= 0 && screen.x <= 800 && screen.y >= 0 && screen.y <= 450;
    });
    const picksBack = () => {
      const screen = rig.projectToViewport(600, 0, 120, { x: 0, y: 0 });
      const picked = rig.clientToField(screen.x, screen.y, rect);
      return picked !== null && Math.hypot(picked.x - 600, picked.y - 120) < 1e-3;
    };
    const sameBounds = () => ["minX", "minY", "maxX", "maxY"].every((key) => rig.fieldBounds[key] === initialBounds[key]);
    for (const [delta, direction] of [[-1, "straight down"], [2, "toward the horizon"]]) {
      const moved = rig.tiltBy(delta);
      check(moved && !rig.tiltBy(delta) && rig.tilt !== initialTilt, `Board tilt clamps ${direction}`);
      check(framesField() && picksBack() && sameBounds(), `Board tilt ${direction} keeps the field framed, picking exact, and gameplay bounds fixed`);
      rig.resize(800, 450);
      check(sameBounds() && framesField(), `Board tilt ${direction} survives a resize`);
    }

    const fieldAt = (x, y) => rig.clientToField(x, y, rect);
    const nearPoint = (a, b) => a !== null && b !== null && Math.hypot(a.x - b.x, a.y - b.y) < 1e-3;
    check(rig.resetView() && !rig.resetView() && rig.tilt === initialTilt && framesField(), "Board view reset restores the default framing");
    check(!rig.panBetween(400, 225, 300, 200, rect), "Board pan is locked while the whole field is framed");
    const cursorGround = fieldAt(620, 140);
    check(rig.zoomAt(2, 620, 140, rect) && nearPoint(fieldAt(620, 140), cursorGround), "Board zoom keeps the ground under the cursor in place");
    const grabbed = fieldAt(300, 200);
    check(rig.panBetween(300, 200, 360, 250, rect) && nearPoint(fieldAt(360, 250), grabbed), "Board pan keeps the grabbed ground under the cursor");
    check(picksBack() && sameBounds(), "Zoomed and panned board keeps picking exact and gameplay bounds fixed");
    rig.panBetween(400, 225, 400 + 5000, 225 + 5000, rect);
    const center = fieldAt(400, 225);
    check(!rig.panBetween(400, 225, 500, 300, rect) && center.x > 0 && center.x < fieldWidth / 2 && center.y > 0 && center.y < fieldHeight / 2, "Board pan stops at the field edge");
    check(rig.zoomAt(100, 400, 225, rect) && rig.zoomFactor === 4 && !rig.zoomAt(2, 400, 225, rect), "Board zoom clamps at 4x");
    rig.zoomAt(1e-3, 400, 225, rect);
    check(rig.zoomFactor === 1 && framesField(), "Zooming fully out re-frames the whole field and drops the pan");
    rig.resize(800, 450);
    check(sameBounds() && framesField(), "Board view survives a resize");
  }

  const droneGame = makeGame(DESKTOP_GAME_PROFILE);
  droneGame.startLevelByIndex(0); droneGame.runtime.spawnDelay = 999;
  const targets = [100, 130].map(x => createMonster("square", [{ x, y: 100, totalDistance: 0 }, { x: 900, y: 100, totalDistance: 900 - x }], 0, 0));
  droneGame.runtime.monsters.push(...targets);
  droneGame.runtime.drones.push(new Drone({ x: 60, y: 100 }, 0), new Drone({ x: 60, y: 100 }, 0));
  droneGame.updateSimulation(1 / 60);
  check(new Set(droneGame.runtime.drones.map(drone => drone.getAssignedTarget())).size === 2, "Drones retargeting in one step account for each other's assignments");

  const capped = new UpdateResult(); capped.particleLimit = 0;
  createPolygonShardParticles(capped, target, [], { x: 0, y: 0 }, 0, 1, 2, 0, { splitIntoShards() { throw new Error("Full particle budget must skip polygon splitting"); } });
  check(capped.particles.length === 0, "Exhausted particle capacity skips polygon construction");
  const cappedLinks = new UpdateResult(); cappedLinks.linkLimit = 0;
  const slowTarget = createMonster("square", path, 1, 0);
  const slow = new SlowTower(60, 100);
  slow.update({ ...context, activeMonsters: [slowTarget] }, cappedLinks);
  check(cappedLinks.links.length === 0 && slowTarget.speedPerSecond < slowTarget.maxSpeedPerSecond && cappedLinks.sounds.length === 1, "Full link capacity preserves slowing and sound");
  cappedLinks.clear(); cappedLinks.linkLimit = 0;
  const lightning = new LightningTower(60, 100);
  const hitPointsBefore = slowTarget.hitPoints;
  lightning.update({ ...context, activeMonsters: [slowTarget] }, cappedLinks);
  check(cappedLinks.links.length === 0 && slowTarget.hitPoints < hitPointsBefore && cappedLinks.sounds.length === 1, "Full link capacity preserves lightning damage and sound");
  for (const limit of [0, 1, 2, 5]) {
    const recipes = [createHitImpactParticles(0, 0, "#ffffff", 0, limit), createLaserImpactParticles(0, 0, 0, "#ffffff", limit), createMissileExplosionParticles(0, 0, 0, 0, limit), createEscapeBurstParticles(0, 0, ESCAPE_BURST_CONFIG, limit)];
    check(recipes.every(particles => particles.length <= limit), `Effect recipes respect a ${limit}-particle budget`);
  }
  const crowdedGame = makeGame(DESKTOP_GAME_PROFILE), sounds = [];
  crowdedGame.audio = { play(cue) { sounds.push(cue.id); } };
  crowdedGame.startLevelByIndex(0); crowdedGame.runtime.spawnDelay = 999;
  crowdedGame.runtime.particles = Array.from({ length: 2000 }, () => ({ removed: false, update() {} }));
  const splitter = createMonster("splitter", path, 1, 0), moneyBefore = crowdedGame.runtime.money;
  splitter.hitPoints = 0; crowdedGame.runtime.monsters.push(splitter);
  crowdedGame.updateSimulation(1 / 60);
  check(crowdedGame.runtime.particles.length === 2000 && crowdedGame.runtime.money === moneyBefore + splitter.bounty && crowdedGame.runtime.monsters.length === 2 && sounds.includes("splitter-burst"), "Full particle capacity preserves splitter children, bounty, and sound");
  const armored = createMonster("bulwark", [{ x: 400, y: 300, totalDistance: 0 }, { x: 700, y: 300, totalDistance: 300 }], 0, 0);
  crowdedGame.runtime.monsters.push(armored);
  crowdedGame.runtime.projectiles.push(new GunProjectile(armored, { x: 410, y: 300 }, 0));
  crowdedGame.updateSimulation(1 / 60);
  check(armored.hitPoints < armored.maxHitPoints && crowdedGame.runtime.particles.length === 2000 && sounds.includes("projectile-impact"), "Full particle capacity preserves projectile damage and impact sound");

  for (const failedMethod of ["getItem", "setItem", "removeItem"]) {
    const data = new Map(); let fail = false;
    const storage = Object.fromEntries(["getItem", "setItem", "removeItem"].map(method => [method, (key, value) => {
      if (fail && method === failedMethod) throw new Error("Storage unavailable");
      if (method === "getItem") return data.get(key) ?? null;
      if (method === "setItem") data.set(key, value); else data.delete(key);
    }]));
    const store = new CampaignProgressStore(storage);
    store.saveCampaignProgress(3, true); store.saveLevelStars(2, 3); store.loadCampaignProgress(10);
    fail = true;
    if (failedMethod === "getItem") store.loadCampaignProgress(10);
    store.saveCampaignProgress(4, false);
    const progress = store.loadCampaignProgress(10);
    check(progress.highestUnlockedLevelIndex === 4 && !progress.campaignCleared && store.loadLevelStars(10)[2] === 3, `Progress survives ${failedMethod} failure using memory`);
  }

  const session = createGameSession(DESKTOP_GAME_PROFILE);
  const originalDraw = Game.prototype.draw, originalUnlock = GameAudio.prototype.unlock, originalPlay = GameAudio.prototype.play;
  let draws = 0, paintedPlacement;
  Game.prototype.draw = function () {
    draws++;
    paintedPlacement = this.runtime.placingTower;
    originalDraw.call(this);
  };
  GameAudio.prototype.unlock = GameAudio.prototype.play = () => {};
  try {
    const canvas = document.createElement("canvas"), overlay = document.createElement("canvas");
    for (const element of [canvas, overlay]) element.style.cssText = "width:960px;height:480px";
    document.body.append(canvas, overlay);
    session.mount({ canvas, overlay });
    let rendererStatus;
    const unsubscribe = session.rendererStatus.subscribe(value => { rendererStatus = value; });
    for (let waited = 0; rendererStatus === "loading" && waited < 30_000; waited += 50) await new Promise(resolve => setTimeout(resolve, 50));
    unsubscribe();
    check(rendererStatus === "ready", "WebGPU board renderer initializes and attaches");
    session.selectLevel(0);
    session.toggleTowerPlacement("gun");
    await new Promise(requestAnimationFrame);
    check(paintedPlacement === "gun", "Placement is painted before pause");
    const beforePause = draws;
    session.togglePause();
    check(draws === beforePause + 1 && paintedPlacement === undefined, "Pausing immediately paints the cleared placement");
  } finally {
    session.destroy();
    Game.prototype.draw = originalDraw;
    GameAudio.prototype.unlock = originalUnlock;
    GameAudio.prototype.play = originalPlay;
  }
  return passed;
}));
for (const viewport of [{ width: 1200, height: 900 }, { width: 375, height: 812 }, { width: 390, height: 1000 }]) {
  await runBrowserPage({ path: "/", viewport }, async page => {
    const dialog = page.getByRole("dialog");
    await dialog.getByRole("button", { name: "Play Next", exact: true }).click();
    const campaign = page.getByRole("button", { name: "Campaign", exact: true });
    await campaign.click();
    const containsFocus = () => dialog.evaluate(element => element.contains(document.activeElement));
    if (!await containsFocus()) throw new Error("Opening map must move focus into the dialog");
    const buttons = dialog.locator("button:enabled");
    await buttons.first().focus(); await page.keyboard.press("Shift+Tab");
    if (!await buttons.last().evaluate(element => element === document.activeElement)) throw new Error("Shift+Tab must wrap inside the modal");
    await page.keyboard.press("Tab");
    if (!await buttons.first().evaluate(element => element === document.activeElement)) throw new Error("Tab must wrap inside the modal");
    await dialog.getByRole("button", { name: "Resume Battle", exact: true }).click();
    if (!await campaign.evaluate(element => element === document.activeElement)) throw new Error("Closing map must restore focus to its opener");
    await page.keyboard.press("j");
    await page.getByRole("dialog", { name: "Level Clear", exact: true }).waitFor();
    await dialog.getByRole("button", { name: "Campaign Map", exact: true }).click();
    if (!await containsFocus()) throw new Error("Switching modal content must retain focus inside it");
  });
  checks.push(`${viewport.width}x${viewport.height}: modal focus, tab containment, restoration, and content transition`);
}
console.log(`${checks.length} runtime checks passed`);
for (const check of checks) console.log(`  ${check}`);
