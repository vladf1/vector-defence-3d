//! Simulation-level checks ported from `scripts/check-runtime.mjs`, plus campaign smoke tests.
use std::collections::HashSet;
use std::f64::consts::PI;

use serde_json::Value;

use crate::audio::AudioCue;
use crate::collision::{
    ActiveCircleSweep, ActiveCircleSweepCollisionIndex, ActiveCircleSweepCollisionQuery, CircleSweep,
};
use crate::combat_effects::{
    ESCAPE_BURST_CONFIG, create_escape_burst_particles, create_hit_impact_particles, create_laser_impact_particles,
    create_missile_explosion_particles,
};
use crate::entities::effects::particle::ParticleMotion;
use crate::entities::monsters::death_effect_helpers::{ShardSource, create_polygon_shard_particles};
use crate::entities::monsters::polygon_shard_splitter::{PolygonShardSplitter, PolygonShardSplitterConfig};
use crate::entities::projectiles::missile::create_missile_visual;
use crate::entities::{Drone, Missile, MonsterRef, Particle, Projectile, Tower, TowerRef, share};
use crate::game::{CanvasActionHit, Game};
use crate::monster_factory::{create_monster, create_splitter_children};
use crate::profile::GameMode;
use crate::progress::MemoryProgressStorage;
use crate::rng;
use crate::route_path::{SharedPath, create_path_entries};
use crate::types::{FieldBounds, GameState, ModalAction, MonsterKind, Point, TowerKind};
use crate::update::{StandaloneUpdateContext, UpdateResult};
use crate::utils::{calculate_intercept, is_within_distance_to_segment, random_range};
use crate::view::{RuntimeHudStats, create_hud_json, create_modal_json, tower_catalog_json};

const HZ_RATES: [f64; 4] = [60.0, 120.0, 144.0, 240.0];

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-7
}

fn path(points: &[(f64, f64)]) -> SharedPath {
    let points: Vec<Point> = points.iter().map(|&(x, y)| Point::new(x, y)).collect();
    create_path_entries(&points).into()
}

fn test_path() -> SharedPath {
    path(&[(100.0, 100.0), (1000.0, 100.0)])
}

fn monster(kind: MonsterKind, route: SharedPath, speed_scale: f64) -> MonsterRef {
    share(create_monster(kind, route, speed_scale, 0))
}

fn standalone(monsters: Vec<MonsterRef>) -> StandaloneUpdateContext {
    StandaloneUpdateContext::new(
        1200.0,
        600.0,
        FieldBounds { min_x: 0.0, min_y: 0.0, max_x: 1200.0, max_y: 600.0 },
        monsters,
    )
}

fn new_game(mode: GameMode) -> Game {
    Game::new(mode, Box::new(MemoryProgressStorage::default()))
}

/// A stationary square at the test path's start.
fn still_target() -> MonsterRef {
    let target = monster(MonsterKind::Square, test_path(), 1.0);
    {
        let mut target = target.borrow_mut();
        target.velocity_x_per_second = 0.0;
        target.velocity_y_per_second = 0.0;
    }
    target
}

/// Frames like the TS `for (frame = 0; frame < hz * seconds; frame++)` loop.
fn frames(hz: f64, seconds: f64) -> usize {
    let mut count = 0;
    while (count as f64) < hz * seconds {
        count += 1;
    }
    count
}

fn count_tower_shots(tower: &TowerRef, context: &StandaloneUpdateContext, seconds: f64, hz: f64) -> usize {
    let mut result = UpdateResult::new();
    let mut shots = 0;
    for _ in 0..frames(hz, seconds) {
        result.clear();
        Tower::update(tower, &context.context(1.0 / hz), &mut result);
        shots += result.projectiles.len();
    }
    shots
}

fn aimed_gun(x: f64, y: f64) -> TowerRef {
    let gun = share(Tower::new(TowerKind::Gun, x, y));
    {
        let mut gun = gun.borrow_mut();
        gun.set_angle(0.0);
        gun.range = 200.0;
    }
    gun
}

#[test]
fn weapon_fire_rates_do_not_depend_on_refresh_rate() {
    rng::seed(42);
    let context = standalone(vec![still_target()]);
    for hz in HZ_RATES {
        let gun = aimed_gun(0.0, 100.0);
        assert_eq!(count_tower_shots(&gun, &context, 60.0, hz), 300, "gun at {hz} Hz");

        let drone = share(Drone::new(Point::new(60.0, 100.0), 6));
        let mut result = UpdateResult::new();
        let mut drone_shots = 0;
        for _ in 0..frames(hz, 10.0) {
            result.clear();
            drone.borrow_mut().update(&context.context(1.0 / hz), &mut result);
            drone_shots += result.projectiles.len();
        }
        assert_eq!(drone_shots, 27, "drone at {hz} Hz");
    }
}

#[test]
fn spawning_does_not_depend_on_refresh_rate() {
    rng::seed(42);
    for hz in HZ_RATES {
        let mut game = new_game(GameMode::Desktop);
        game.start_level_by_index(0);
        game.runtime.spawn_delay = 0.0;
        {
            let wave = game.runtime.active_wave_mut().unwrap();
            wave.count = 1000;
            wave.spawn_interval_min = 0.23;
            wave.spawn_interval_max = 0.23;
        }
        game.runtime.escapes_left = 1000;
        for _ in 0..frames(hz, 10.0) {
            game.update_simulation(1.0 / hz);
        }
        assert_eq!(game.runtime.spawned_monsters, 43, "spawning at {hz} Hz");
    }
}

#[test]
fn idle_weapons_do_not_bank_shots() {
    rng::seed(42);
    let idle_gun = aimed_gun(0.0, 100.0);
    let empty = standalone(Vec::new());
    let mut result = UpdateResult::new();
    for _ in 0..600 {
        Tower::update(&idle_gun, &empty.context(1.0 / 60.0), &mut result);
    }
    let context = standalone(vec![still_target()]);
    assert_eq!(count_tower_shots(&idle_gun, &context, 0.2, 60.0), 1);
}

#[test]
fn hit_shake_leaves_position_and_swept_collision_unchanged() {
    rng::seed(42);
    let target = still_target();
    let targets = vec![target.clone()];
    let mut index = ActiveCircleSweepCollisionIndex::new(64.0);
    index.rebuild(&targets);
    let sweep = CircleSweep { previous_x: 50.0, previous_y: 100.0, x: 150.0, y: 100.0, radius: 1.0 };
    let collision_time = index.find_earliest_collision(&sweep, &targets).unwrap().time;
    target.borrow_mut().shake_from_hit();
    let after = index.find_earliest_collision(&sweep, &targets).unwrap().time;
    let target = target.borrow();
    assert!(target.x == 100.0 && target.y == 100.0 && after == collision_time);
    assert!(target.visual_x() != target.x || target.visual_y() != target.y);
}

#[test]
fn death_particles_match_the_visual_shake_offset() {
    for kind in MonsterKind::ALL {
        let monster = monster(kind, test_path(), 1.0);
        rng::seed(42);
        let mut before = UpdateResult::new();
        monster.borrow().add_death_effect(&mut before);
        monster.borrow_mut().shake_from_hit();
        let (dx, dy) = {
            let monster = monster.borrow();
            (monster.visual_x() - monster.x, monster.visual_y() - monster.y)
        };
        rng::seed(42);
        let mut after = UpdateResult::new();
        monster.borrow().add_death_effect(&mut after);
        assert!(!before.particles.is_empty(), "{kind:?} breaks into shards");
        assert_eq!(before.particles.len(), after.particles.len(), "{kind:?}");
        for (first, second) in before.particles.iter().zip(&after.particles) {
            assert!(near(second.x - first.x, dx) && near(second.y - first.y, dy), "{kind:?}");
        }
        assert_eq!(before.sounds.len(), 1, "{kind:?} plays one death sound");
    }
}

#[test]
fn intercepts_lead_moving_targets() {
    for (velocity, expected) in [(10.0, 125.0), (-50.0, 50.0), (-100.0, 100.0 / 3.0), (100.0, 100.0)] {
        let intercept = calculate_intercept(Point::new(100.0, 0.0), velocity, 0.0, 50.0, Point::new(0.0, 0.0));
        assert!(near(intercept.x, expected), "velocity {velocity}");
    }
}

#[test]
fn laser_endpoint_follows_the_heading_in_tracked_and_locked_modes() {
    rng::seed(42);
    let context = standalone(vec![still_target()]);
    let laser = share(Tower::new(TowerKind::Laser, 0.0, 100.0));
    {
        let mut laser = laser.borrow_mut();
        laser.set_angle(0.0);
        laser.range = 200.0;
    }
    let mut result = UpdateResult::new();
    Tower::update(&laser, &context.context(1.0 / 60.0), &mut result);
    laser.borrow_mut().laser_mut().unwrap().direction_locked = true;
    Tower::update(&laser, &context.context(1.0 / 60.0), &mut result);
    let endpoint = laser.borrow().laser().unwrap().beam_target;
    assert!(near(endpoint.x, 1000.0) && near(endpoint.y, 100.0));
}

fn parse(json: &str) -> Value {
    serde_json::from_str(json).unwrap_or_else(|error| panic!("invalid JSON ({error}): {json}"))
}

#[test]
fn selection_labels_and_star_award_copy() {
    rng::seed(42);
    for mode in [GameMode::Desktop, GameMode::Mobile] {
        let mut game = new_game(mode);
        game.start_level_by_index(0);
        let tower = share(game.create_tower(TowerKind::Gun, Point::new(50.0, 100.0)));
        game.runtime.selected_tower = Some(tower);
        let hud = parse(&create_hud_json(&game, &RuntimeHudStats::default()));
        let expected = if mode == GameMode::Mobile { "Gun Tower" } else { "Gun Tower · Level 1 · Range 60" };
        assert_eq!(hud["selectionName"], expected);
        assert_eq!(hud["upgradeLabel"], "Upgrade - $5");
        if mode == GameMode::Mobile {
            assert_eq!(hud["selectionSummary"], "Level 1 · Range 54");
        }
        game.finish_level();
        let modal = parse(&create_modal_json(&game).unwrap());
        assert_eq!(modal["starAward"]["title"], "Perfect route");
        assert_eq!(game.level_stars[0], 3);
        assert_eq!(game.highest_unlocked_level_index, 1);
    }
}

#[test]
fn first_projectile_activates_collision_queries() {
    rng::seed(42);
    for mode in [GameMode::Desktop, GameMode::Mobile] {
        let mut game = new_game(mode);
        game.start_level_by_index(0);
        game.runtime.spawn_delay = 999.0;
        let target = monster(MonsterKind::Square, test_path(), 1.0);
        game.runtime.monsters.push(target.clone());
        game.update_simulation(1.0 / 60.0);
        // No projectiles: the collision index can stay dormant.
        let (x, y) = {
            let target = target.borrow();
            (target.x, target.y)
        };
        let shot = Projectile::gun(Point::new(x, y), Point::new(x + 1.0, y), 0);
        let shot_id = shot.id;
        game.runtime.projectiles.push(shot);
        game.update_simulation(1.0 / 60.0);
        assert!(game.runtime.projectiles.iter().all(|projectile| projectile.id != shot_id), "{mode:?}");
        let target = target.borrow();
        assert!(target.hit_points < target.max_hit_points, "{mode:?}");
    }
}

#[test]
fn beam_intersections_match_segment_geometry() {
    rng::seed(7);
    let beam = share(Tower::new(TowerKind::Laser, 0.0, 100.0));
    let mut result = UpdateResult::new();
    for iteration in 0..200 {
        {
            let mut beam = beam.borrow_mut();
            beam.set_angle(random_range(-PI, PI));
            beam.cooldown_seconds = 10.0;
            let laser = beam.laser_mut().unwrap();
            laser.direction_locked = true;
            laser.beam_alpha = 1.0;
        }
        let sample = monster(MonsterKind::Square, test_path(), 0.0);
        {
            let mut sample = sample.borrow_mut();
            sample.x = random_range(-1100.0, 1100.0);
            sample.y = random_range(-1000.0, 1200.0);
            sample.radius = random_range(1.0, 100.0);
        }
        let context = standalone(vec![sample.clone()]);
        result.clear();
        Tower::update(&beam, &context.context(1.0 / 60.0), &mut result);
        let beam = beam.borrow();
        let source = Point::new(beam.x + beam.angle().cos() * 8.5, beam.y + beam.angle().sin() * 8.5);
        let sample = sample.borrow();
        let expected = is_within_distance_to_segment(
            Point::new(sample.x, sample.y),
            source,
            beam.laser().unwrap().beam_target,
            sample.radius,
        );
        assert_eq!(sample.hit_points < sample.max_hit_points, expected, "beam query {iteration}");
    }
}

#[test]
fn laser_fade_integrates_to_the_same_damage_at_any_rate() {
    rng::seed(42);
    for hz in HZ_RATES {
        let victim = monster(MonsterKind::Bulwark, test_path(), 1.0);
        let laser = share(Tower::new(TowerKind::Laser, 0.0, 100.0));
        {
            let mut laser = laser.borrow_mut();
            laser.set_angle(0.0);
            laser.cooldown_seconds = 10.0;
            let state = laser.laser_mut().unwrap();
            state.direction_locked = true;
            state.beam_alpha = 1.0;
        }
        let context = standalone(vec![victim.clone()]);
        let mut result = UpdateResult::new();
        for _ in 0..hz as usize {
            Tower::update(&laser, &context.context(1.0 / hz), &mut result);
        }
        let victim = victim.borrow();
        assert!(near(victim.max_hit_points - victim.hit_points, 33.0), "laser through armor at {hz} Hz");
    }
}

#[test]
fn expanded_board_bounds_keep_shots_alive() {
    rng::seed(42);
    let cases = [
        (
            GameMode::Mobile,
            FieldBounds { min_x: 0.0, min_y: -80.0, max_x: 390.0, max_y: 680.0 },
            Point::new(195.0, 600.0),
            Point::new(195.0, 540.0),
        ),
        (
            GameMode::Desktop,
            FieldBounds { min_x: -120.0, min_y: 0.0, max_x: 920.0, max_y: 450.0 },
            Point::new(-40.0, 225.0),
            Point::new(40.0, 225.0),
        ),
    ];
    for (mode, bounds, source, destination) in cases {
        let mut game = new_game(mode);
        game.set_visible_field_bounds(bounds);
        game.start_level_by_index(0);
        game.runtime.spawn_delay = 999.0;
        assert!(game.can_place_tower(source), "{mode:?}: expanded board permits the test placement");
        assert!(!game.can_place_tower_in_bounds(source, game.profile.placement.bounds));
        let victim = monster(MonsterKind::Square, path(&[(destination.x, destination.y), (900.0, destination.y)]), 0.0);
        game.runtime.monsters.push(victim.clone());
        let shot = Projectile::gun(source, destination, 0);
        let shot_id = shot.id;
        let missile = Missile::new(source, victim.clone(), 0, &create_missile_visual(0), None);
        let missile_id = missile.id;
        game.runtime.projectiles.push(shot);
        game.runtime.missiles.push(missile);
        game.update_simulation(1.0 / 60.0);
        assert!(game.runtime.projectiles.iter().any(|shot| shot.id == shot_id), "{mode:?}: shot survives");
        assert!(game.runtime.missiles.iter().any(|missile| missile.id == missile_id), "{mode:?}: missile survives");
        for _ in 0..30 {
            game.update_simulation(1.0 / 60.0);
        }
        assert!(game.runtime.projectiles.iter().all(|shot| shot.id != shot_id), "{mode:?}: shot lands");
        {
            let victim = victim.borrow();
            assert!(victim.hit_points < victim.max_hit_points, "{mode:?}: shot damages its target");
        }

        game.open_menu();
        game.open_menu();
        game.resume_battle();
        assert_eq!(game.state, GameState::Playing, "{mode:?}: repeated map opening preserves resume state");
        game.toggle_pause();
        game.open_menu();
        game.open_menu();
        game.resume_battle();
        assert_eq!(game.state, GameState::Paused, "{mode:?}: repeated map opening preserves paused state");
    }
}

#[test]
fn drones_retargeting_in_one_step_account_for_each_other() {
    rng::seed(42);
    let mut game = new_game(GameMode::Desktop);
    game.start_level_by_index(0);
    game.runtime.spawn_delay = 999.0;
    for x in [100.0, 130.0] {
        game.runtime.monsters.push(monster(MonsterKind::Square, path(&[(x, 100.0), (900.0, 100.0)]), 0.0));
    }
    for _ in 0..2 {
        game.runtime.drones.push(share(Drone::new(Point::new(60.0, 100.0), 0)));
    }
    game.update_simulation(1.0 / 60.0);
    let targets: HashSet<u32> = game
        .runtime
        .drones
        .iter()
        .filter_map(|drone| drone.borrow().get_assigned_target().map(|target| target.borrow().id))
        .collect();
    assert_eq!(targets.len(), 2);
}

#[test]
fn exhausted_particle_capacity_skips_polygon_construction() {
    let target = still_target();
    let mut capped = UpdateResult::new();
    capped.particle_limit = 0;
    rng::seed(5);
    let expected_next = {
        rng::seed(5);
        let value = rng::random();
        rng::seed(5);
        value
    };
    let splitter = PolygonShardSplitter::new(PolygonShardSplitterConfig::with_shard_counts(5, 11));
    let target = target.borrow();
    let outline = [Point::new(-5.0, -5.0), Point::new(5.0, -5.0), Point::new(5.0, 5.0), Point::new(-5.0, 5.0)];
    create_polygon_shard_particles(
        &mut capped,
        ShardSource { visual_x: target.visual_x(), visual_y: target.visual_y(), color: target.color },
        &outline,
        Point::new(0.0, 0.0),
        0.0,
        1.0,
        2.0,
        0.0,
        &splitter,
    );
    assert!(capped.particles.is_empty());
    assert_eq!(rng::random(), expected_next, "no randomness consumed: the splitter never ran");
}

#[test]
fn full_link_capacity_preserves_slowing_damage_and_sound() {
    rng::seed(42);
    let slow_target = monster(MonsterKind::Square, test_path(), 1.0);
    let context = standalone(vec![slow_target.clone()]);
    let mut capped = UpdateResult::new();
    capped.link_limit = 0;
    let slow = share(Tower::new(TowerKind::Slow, 60.0, 100.0));
    Tower::update(&slow, &context.context(1.0 / 60.0), &mut capped);
    {
        let target = slow_target.borrow();
        assert!(capped.links.is_empty() && target.speed_per_second < target.max_speed_per_second);
        assert_eq!(capped.sounds.len(), 1);
    }

    capped.clear();
    capped.link_limit = 0;
    let lightning = share(Tower::new(TowerKind::Lightning, 60.0, 100.0));
    let hit_points_before = slow_target.borrow().hit_points;
    Tower::update(&lightning, &context.context(1.0 / 60.0), &mut capped);
    assert!(capped.links.is_empty() && slow_target.borrow().hit_points < hit_points_before);
    assert_eq!(capped.sounds.len(), 1);
}

#[test]
fn links_are_created_and_fade() {
    rng::seed(42);
    let targets: Vec<MonsterRef> = [100.0, 140.0, 180.0]
        .iter()
        .map(|&x| monster(MonsterKind::Square, path(&[(x, 100.0), (900.0, 100.0)]), 1.0))
        .collect();
    let context = standalone(targets.clone());
    let lightning = share(Tower::new(TowerKind::Lightning, 60.0, 100.0));
    for _ in 0..4 {
        lightning.borrow_mut().upgrade();
    }
    let mut result = UpdateResult::new();
    Tower::update(&lightning, &context.context(1.0 / 60.0), &mut result);
    // Level 4 chains to 2 + 4 / 2 = 4 targets; only three are present.
    assert_eq!(result.links.len(), 3);
    assert_eq!(result.links[0].source.level(), Some(4));
    assert_eq!(result.links[1].source.level(), None);
    let mut link = result.links.remove(0);
    for _ in 0..30 {
        link.update(&context.context(1.0 / 60.0));
    }
    assert!(link.removed, "lightning links fade in about a quarter second");
}

#[test]
fn effect_recipes_respect_the_particle_budget() {
    rng::seed(42);
    for limit in [0, 1, 2, 5] {
        assert!(create_hit_impact_particles(0.0, 0.0, 0xffffff, Some(0.0), limit).len() <= limit);
        assert!(create_laser_impact_particles(0.0, 0.0, 0.0, 0xffffff, limit).len() <= limit);
        assert!(create_missile_explosion_particles(0.0, 0.0, 0.0, 0, limit).len() <= limit);
        assert!(create_escape_burst_particles(0.0, 0.0, &ESCAPE_BURST_CONFIG, limit).len() <= limit);
    }
    assert_eq!(create_escape_burst_particles(0.0, 0.0, &ESCAPE_BURST_CONFIG, usize::MAX).len(), 3 + 58 + 30 + 18);
    assert_eq!(create_missile_explosion_particles(0.0, 0.0, 0.0, 3, usize::MAX).len(), 25);
}

fn still_particle() -> Particle {
    Particle::spark(
        400.0,
        300.0,
        1.0,
        0xffffff,
        0.0,
        ParticleMotion { speed_per_second: 0.0, offset: 0.0, angle: Some(0.0) },
    )
}

#[test]
fn full_particle_capacity_preserves_gameplay() {
    rng::seed(42);
    let mut game = new_game(GameMode::Desktop);
    game.start_level_by_index(0);
    game.runtime.spawn_delay = 999.0;
    game.runtime.particles = (0..2000).map(|_| still_particle()).collect();
    game.take_sounds();
    let splitter = monster(MonsterKind::Splitter, test_path(), 1.0);
    splitter.borrow_mut().hit_points = 0.0;
    let money_before = game.runtime.money;
    game.runtime.monsters.push(splitter.clone());
    game.update_simulation(1.0 / 60.0);
    let sounds: Vec<AudioCue> = game.take_sounds().iter().map(|sound| sound.cue).collect();
    assert_eq!(game.runtime.particles.len(), 2000);
    assert_eq!(game.runtime.money, money_before + splitter.borrow().bounty);
    assert_eq!(game.runtime.monsters.len(), 2);
    assert!(sounds.contains(&AudioCue::SplitterBurst));

    let armored = monster(MonsterKind::Bulwark, path(&[(400.0, 300.0), (700.0, 300.0)]), 0.0);
    game.runtime.monsters.push(armored.clone());
    game.runtime.projectiles.push(Projectile::gun(Point::new(400.0, 300.0), Point::new(410.0, 300.0), 0));
    game.update_simulation(1.0 / 60.0);
    let sounds: Vec<AudioCue> = game.take_sounds().iter().map(|sound| sound.cue).collect();
    let armored = armored.borrow();
    assert!(armored.hit_points < armored.max_hit_points);
    assert_eq!(game.runtime.particles.len(), 2000);
    assert!(sounds.contains(&AudioCue::ProjectileImpact));
}

#[test]
fn splitter_children_are_weakened_runners_around_the_split() {
    rng::seed(42);
    let splitter = create_monster(MonsterKind::Splitter, test_path(), 1.0, 3);
    let mut splitter = splitter;
    splitter.distance_along_path = 400.0;
    splitter.x = 500.0;
    let children = create_splitter_children(&splitter, 1.0, 3);
    assert_eq!(children.len(), 2);
    let runner = create_monster(MonsterKind::Runner, test_path(), 1.0, 3);
    for (index, child) in children.iter().enumerate() {
        assert_eq!(child.kind(), MonsterKind::Runner);
        assert_eq!(child.hit_points, (runner.max_hit_points * 0.72 + 0.5).floor());
        assert_eq!(child.max_hit_points, child.hit_points);
        assert_eq!(child.bounty, 1);
        assert!(near(child.radius, runner.radius * 0.86));
        assert!(child.max_speed_per_second < runner.max_speed_per_second);
        // The first child starts 10-18 behind the split, the second 10-18 ahead.
        let start = child.path[0].x;
        if index == 0 {
            assert!((482.0..=490.0).contains(&start), "{start}");
        } else {
            assert!((510.0..=518.0).contains(&start), "{start}");
        }
        assert!(near(child.path.last().unwrap().x, 1000.0));
    }
}

#[test]
fn splitter_near_the_route_start_spawns_both_children_ahead() {
    rng::seed(42);
    let splitter = create_monster(MonsterKind::Splitter, test_path(), 1.0, 0);
    let children = create_splitter_children(&splitter, 1.0, 0);
    assert!(children.iter().all(|child| child.path[0].x >= 110.0));
}

#[test]
fn bulwark_armor_applies_to_discrete_hits_only() {
    rng::seed(42);
    let mut bulwark = create_monster(MonsterKind::Bulwark, test_path(), 1.0, 0);
    let full = bulwark.hit_points;
    bulwark.take_damage(10.0);
    assert!(near(full - bulwark.hit_points, 6.5));
    bulwark.take_damage(1.0);
    assert!(near(full - bulwark.hit_points, 6.9));
    bulwark.take_damage(0.0);
    assert!(near(full - bulwark.hit_points, 6.9));
    bulwark.take_continuous_damage(10.0);
    assert!(near(full - bulwark.hit_points, 16.9));

    let mut square = create_monster(MonsterKind::Square, test_path(), 1.0, 0);
    let full = square.hit_points;
    square.take_damage(10.0);
    assert!(near(full - square.hit_points, 10.0));
}

#[test]
fn level_scaling_raises_hit_points() {
    rng::seed(42);
    let base = create_monster(MonsterKind::Tank, test_path(), 1.0, 0);
    let scaled = create_monster(MonsterKind::Tank, test_path(), 1.0, 5);
    assert!(near(scaled.max_hit_points, base.max_hit_points * 1.3));
    assert!(near(scaled.hit_points, scaled.max_hit_points));
}

#[test]
fn monsters_escape_at_the_end_of_their_path() {
    rng::seed(42);
    let runner = monster(MonsterKind::Runner, path(&[(0.0, 0.0), (10.0, 0.0)]), 1.0);
    let context = standalone(Vec::new());
    let mut result = UpdateResult::new();
    crate::entities::Monster::update(&runner, &context.context(0.5), &mut result);
    assert_eq!(result.escaped_monsters.len(), 1);
    assert!(runner.borrow().removed);
    assert_eq!(runner.borrow().get_path_progress(), 1.0);
}

/// Places towers of the level's kinds in a loop around the route until money runs out.
fn build_defense(game: &mut Game, kinds: &[TowerKind], next_kind: &mut usize) {
    let bounds = game.profile.placement.bounds;
    let entries: Vec<Point> =
        game.runtime.route_path.as_ref().unwrap().entries.iter().map(|entry| entry.point()).collect();
    for entry in entries.iter().step_by(3) {
        for (dx, dy) in [(30.0, 30.0), (-30.0, -30.0), (30.0, -30.0), (-30.0, 30.0), (0.0, 34.0), (34.0, 0.0)] {
            let point = Point::new(entry.x + dx, entry.y + dy);
            if point.x < bounds.min_x || point.y < bounds.min_y || point.x > bounds.max_x || point.y > bounds.max_y {
                continue;
            }
            let kind = kinds[*next_kind % kinds.len()];
            if !game.can_afford_tower(kind) {
                return;
            }
            if game.place_tower(kind, point) {
                *next_kind += 1;
            }
        }
    }
}

fn upgrade_everything(game: &mut Game) {
    let towers = game.runtime.towers.clone();
    for tower in towers {
        game.runtime.selected_tower = Some(tower);
        while game.can_upgrade_selected_tower() && game.runtime.money > 12 {
            game.upgrade_selected_tower();
        }
    }
    game.runtime.selected_tower = None;
}

/// Plays a level with a mixed defense until it is won or lost.
fn play_level(game: &mut Game, index: usize) -> GameState {
    game.start_level_by_index(index);
    let kinds = game.current_level().unwrap().available_towers.clone();
    let mut next_kind = 0;
    let stats = RuntimeHudStats { fps: 59.6, frame_time_ms: 16.75, update_time_ms: 0.5, draw_time_ms: 1.25 };
    let mut remaining = 0.0;
    for frame in 0..(60 * 60 * 30) {
        if !game.needs_animation_frame() {
            break;
        }
        if frame % 30 == 0 {
            build_defense(game, &kinds, &mut next_kind);
            upgrade_everything(game);
            parse(&create_hud_json(game, &stats));
        }
        if frame % 97 == 0
            && let Some(tower) = game.runtime.towers.first().cloned()
        {
            let position = tower.borrow().position();
            game.handle_board_click(position, CanvasActionHit::default());
            game.handle_board_click(position, CanvasActionHit { upgrade_button: false, laser_lock_button: true });
        }
        // Frame times vary like a real display; `advance` splits them into bounded substeps.
        let elapsed = [1.0 / 60.0, 1.0 / 144.0, 0.05, 1.0 / 30.0][frame % 4];
        remaining = game.advance(remaining + elapsed);
        game.take_sounds();
        assert!(game.runtime.particles.len() <= crate::constants::MAX_PARTICLES);
        assert!(game.runtime.links.len() <= crate::constants::MAX_LINKS);
    }
    game.state
}

#[test]
fn seeded_campaign_levels_play_to_an_outcome() {
    rng::seed(2026);
    let mut game = new_game(GameMode::Desktop);
    let outcome = play_level(&mut game, 0);
    assert!(matches!(outcome, GameState::Won | GameState::Lost), "{outcome:?}");
    if outcome == GameState::Won {
        assert!(game.last_awarded_stars >= 1);
        assert_eq!(game.highest_unlocked_level_index, 1);
        let modal = parse(&create_modal_json(&game).unwrap());
        assert_eq!(modal["title"], "Level Clear");
    }

    game.unlock_all_levels_for_debug();
    for mode in [GameMode::Desktop, GameMode::Mobile] {
        let mut game = new_game(mode);
        game.unlock_all_levels_for_debug();
        for index in 0..game.levels.len() {
            let outcome = play_level(&mut game, index);
            assert!(matches!(outcome, GameState::Won | GameState::Lost | GameState::CampaignWon), "{outcome:?}");
            parse(&create_modal_json(&game).unwrap());
        }
    }
}

#[test]
fn losing_a_level_resolves_after_the_breach_delay() {
    rng::seed(42);
    let mut game = new_game(GameMode::Desktop);
    game.start_level_by_index(0);
    game.runtime.spawn_delay = 0.0;
    game.runtime.escapes_left = 1;
    let mut steps = 0;
    while game.state == GameState::Playing && steps < 60 * 120 {
        game.update_simulation(1.0 / 60.0);
        steps += 1;
    }
    assert_eq!(game.state, GameState::DefeatPending);
    let hud = parse(&create_hud_json(&game, &RuntimeHudStats::default()));
    assert_eq!(hud["banner"], "Base breached");
    assert!(game.needs_animation_frame());
    for _ in 0..61 {
        game.update_simulation(1.0 / 60.0);
    }
    assert_eq!(game.state, GameState::Lost);
    assert!(!game.needs_animation_frame());
    let modal = parse(&create_modal_json(&game).unwrap());
    assert_eq!(modal["title"], "Defeat");
    assert_eq!(modal["sheet"], true);
    assert!(modal.get("starAward").is_none());
    assert_eq!(modal["actions"][0]["action"], "replay");
    game.perform_modal_action(ModalAction::Replay);
    assert_eq!(game.state, GameState::Playing);
}

/// Object keys, sorted (`JSON.parse` consumers do not depend on key order).
fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    keys
}

fn sorted(mut expected: Vec<&str>) -> Vec<&str> {
    expected.sort_unstable();
    expected
}

#[test]
fn hud_and_modal_json_match_the_typescript_shapes() {
    rng::seed(42);
    let mut game = new_game(GameMode::Desktop);
    let menu = parse(&create_modal_json(&game).unwrap());
    assert_eq!(keys(&menu), sorted(vec!["title", "description", "actions", "levelCards"]));
    assert_eq!(menu["title"], "Campaign Map");
    assert_eq!(menu["description"], "10 campaign battles. Clear one route to unlock the next");
    assert_eq!(menu["actions"], serde_json::json!([{ "action": "play-unlocked", "label": "Play Next" }]));
    let cards = menu["levelCards"].as_array().unwrap();
    assert_eq!(cards.len(), 10);
    assert_eq!(
        keys(&cards[0]),
        sorted(vec![
            "index",
            "unlocked",
            "cleared",
            "current",
            "name",
            "stars",
            "status",
            "title",
            "description",
            "summary",
            "starsLabel"
        ])
    );
    assert_eq!(cards[0]["status"], "Next");
    assert_eq!(cards[1]["status"], "Locked");
    assert_eq!(cards[0]["starsLabel"], "0 stars best clear");
    assert!(!cards[0]["description"].as_str().unwrap().ends_with('.'));
    assert!(cards[0]["summary"].as_str().unwrap().contains(" waves · "));

    let hud = parse(&create_hud_json(&game, &RuntimeHudStats::default()));
    assert_eq!(
        keys(&hud),
        sorted(vec![
            "money",
            "waveTotal",
            "banner",
            "selectionName",
            "selectionSummary",
            "upgradeLabel",
            "upgradeValue",
            "sellLabel",
            "sellValue",
            "upgradeDisabled",
            "upgradeUnaffordable",
            "hasSelectedTower",
            "hasLaserLockAction",
            "laserLocked",
            "laserLockDisabled",
            "sellDisabled",
            "cancelBuildDisabled",
            "canTogglePause",
            "showStatusHud",
            "canSkipBreak",
            "paused",
            "towerButtonsDisabled",
            "availableTowers",
            "affordableTowers",
            "nerdStats",
        ])
    );
    assert_eq!(hud["banner"], "Awaiting orders");
    assert_eq!(hud["selectionSummary"], "Select a tower to view upgrades, range, and sell value.");
    assert_eq!(
        hud["nerdStats"],
        serde_json::json!({
            "fps": "0", "frameTime": "0.0 ms", "updateTime": "0.000 ms", "drawTime": "0.000 ms",
            "trackedObjects": "0", "towers": "0", "hostiles": "0", "shots": "0", "effects": "0"
        })
    );
    assert_eq!(keys(&hud["affordableTowers"]), sorted(vec!["gun", "laser", "missile", "slow", "drone", "lightning"]));

    game.perform_modal_action(ModalAction::PlayUnlocked);
    assert!(create_modal_json(&game).is_none());
    let stats = RuntimeHudStats { fps: 143.5, frame_time_ms: 6.25, update_time_ms: 0.0625, draw_time_ms: 1.0 };
    let hud = parse(&create_hud_json(&game, &stats));
    assert_eq!(hud["levelNumber"], 1);
    assert_eq!(hud["waveCurrent"], 1);
    assert_eq!(hud["waveMonstersSpawned"], 0);
    assert!(hud["waveMonsterTotal"].as_u64().unwrap() > 0);
    assert_eq!(hud["banner"], format!("NEXT WAVE IN {}", game.runtime.spawn_delay.ceil()));
    assert_eq!(hud["canSkipBreak"], true);
    assert_eq!(hud["availableTowers"], serde_json::json!(["gun", "laser", "missile", "slow", "drone"]));
    assert_eq!(hud["nerdStats"]["fps"], "144");
    assert_eq!(hud["nerdStats"]["frameTime"], "6.3 ms");
    assert_eq!(hud["nerdStats"]["updateTime"], "0.063 ms");
    assert_eq!(hud["nerdStats"]["drawTime"], "1.000 ms");

    game.toggle_tower_placement(TowerKind::Missile);
    let hud = parse(&create_hud_json(&game, &stats));
    assert_eq!(hud["placingTower"], "missile");
    assert_eq!(hud["selectionName"], "Placing Missile Tower");
    assert_eq!(hud["selectionSummary"], "Slow launcher with splash damage.");
    assert_eq!(hud["cancelBuildDisabled"], false);

    game.skip_build_break();
    let hud = parse(&create_hud_json(&game, &stats));
    assert_eq!(hud["banner"], "");
    assert_eq!(hud["canSkipBreak"], false);

    game.toggle_pause();
    let hud = parse(&create_hud_json(&game, &stats));
    assert_eq!(hud["paused"], true);
    assert_eq!(hud["banner"], "Paused");
    assert!(hud.get("placingTower").is_none(), "pausing clears placement");

    game.open_menu();
    let menu = parse(&create_modal_json(&game).unwrap());
    assert_eq!(menu["actions"][0], serde_json::json!({ "action": "resume", "label": "Resume Battle" }));
    assert_eq!(menu["levelCards"][0]["current"], true);

    game.current_level_index = 9;
    game.state = GameState::Playing;
    game.finish_level();
    let won = parse(&create_modal_json(&game).unwrap());
    assert_eq!(keys(&won), sorted(vec!["title", "description", "sheet", "starAward", "actions"]));
    assert_eq!(won["title"], "You Won the Campaign");
    assert_eq!(keys(&won["starAward"]), sorted(vec!["stars", "title", "description", "label", "perfect"]));
    assert_eq!(won["starAward"]["label"], "3 stars awarded");
    assert!(game.campaign_cleared);
}

#[test]
fn tower_catalog_lists_the_toolbar() {
    let catalog = parse(&tower_catalog_json());
    let entries = catalog.as_array().unwrap();
    assert_eq!(entries.len(), 6);
    assert_eq!(
        entries[0],
        serde_json::json!({
            "kind": "gun", "label": "Gun", "summary": "Fast, cheap, accurate lead shots.",
            "baseCost": 2, "shortcuts": ["1", "g"]
        })
    );
    assert_eq!(entries[5]["kind"], "lightning");
}

#[test]
fn board_clicks_place_select_upgrade_and_sell() {
    rng::seed(42);
    let mut game = new_game(GameMode::Desktop);
    game.start_level_by_index(0);
    let point = Point::new(170.0, 60.0);
    assert!(game.can_place_tower(point));
    game.start_tower_placement(TowerKind::Laser);
    game.handle_board_click(point, CanvasActionHit::default());
    assert_eq!(game.runtime.towers.len(), 1);
    assert_eq!(game.runtime.money, 32 - 3);
    let tower = game.runtime.selected_tower.clone().unwrap();
    game.take_sounds();
    game.handle_board_click(point, CanvasActionHit { upgrade_button: true, laser_lock_button: false });
    assert_eq!(tower.borrow().level, 1);
    assert_eq!(game.runtime.money, 32 - 3 - 5);
    game.handle_board_click(point, CanvasActionHit { upgrade_button: false, laser_lock_button: true });
    assert!(tower.borrow().direction_locked());
    let cues: Vec<AudioCue> = game.take_sounds().iter().map(|sound| sound.cue).collect();
    assert_eq!(cues, [AudioCue::TowerUpgrade, AudioCue::LaserLockOn]);
    // Clicking the selected tower again deselects it.
    game.handle_board_click(point, CanvasActionHit::default());
    assert!(game.runtime.selected_tower.is_none());
    game.select_tower_at(point);
    game.sell_selected_tower();
    assert!(game.runtime.towers.is_empty() && tower.borrow().removed);
    assert_eq!(game.runtime.money, 32 - 3 - 5 + 6);
    // Too close to the route.
    assert!(!game.place_tower(TowerKind::Gun, Point::new(90.0, 60.0)));
}

#[test]
fn missile_explosions_splash_and_shake() {
    rng::seed(42);
    let targets: Vec<MonsterRef> = [200.0, 230.0, 300.0]
        .iter()
        .map(|&x| monster(MonsterKind::Square, path(&[(x, 100.0), (900.0, 100.0)]), 0.0))
        .collect();
    let context = standalone(targets.clone());
    let mut missile = Missile::new(Point::new(180.0, 100.0), targets[0].clone(), 2, &create_missile_visual(2), None);
    let mut result = UpdateResult::new();
    for _ in 0..30 {
        if missile.removed {
            break;
        }
        missile.update(&context.context(1.0 / 60.0), &mut result);
    }
    assert!(missile.removed);
    let damaged: Vec<bool> = targets
        .iter()
        .map(|target| {
            let target = target.borrow();
            target.hit_points < target.max_hit_points
        })
        .collect();
    assert_eq!(damaged, [true, true, false]);
    assert!(
        result
            .particles
            .iter()
            .any(|particle| matches!(particle.kind, crate::entities::ParticleKind::Shockwave { .. }))
    );
    assert!(result.sounds.iter().any(|sound| sound.cue == AudioCue::MissileExplosion));
    assert!(targets[0].is_active());
}
