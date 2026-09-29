//! Native end-to-end checks: a busy seeded game drawn through the frame composer and the
//! entity views, plus the board camera checks from `scripts/check-runtime.mjs`.
use vd_core::game::Game;
use vd_core::profile::{GameMode, GameProfile};
use vd_core::progress::MemoryProgressStorage;
use vd_core::rng;
use vd_core::types::{FieldBounds, GameState, Point, TowerKind};

use crate::camera_rig::{CameraRig, SurfaceRect, create_board_camera_rig};
use crate::entity_views::{EntityViews, GameScene};
use crate::frame_composer::{FrameComposer, FrameInput};
use crate::render_batches::Batch;

fn frame_input(game: &Game) -> FrameInput<'_> {
    FrameInput {
        animating: game.needs_animation_frame(),
        runtime_id: game.runtime_generation,
        route: game.runtime.route_path.as_ref().map(|route| &route.entries),
        escapes_left: game.runtime.escapes_left,
        simulation_seconds: game.simulation_seconds,
    }
}

fn compose(composer: &mut FrameComposer, views: &mut EntityViews, game: &Game, now: f64) {
    let bounds = composer.rig.field_bounds();
    let input = frame_input(game);
    composer.compose(now, &input, &mut GameScene { game, views, bounds });
}

/// Builds every tower kind across the field and upgrades them.
fn fortify(game: &mut Game) {
    game.runtime.money = 1_000_000;
    let kinds: Vec<TowerKind> = game.current_level().expect("level").available_towers.clone();
    assert!(kinds.len() >= 5, "{kinds:?}");
    let mut placed = 0;
    for row in 0..14 {
        for column in 0..24 {
            let point = Point::new(18.0 + column as f64 * 33.0, 16.0 + row as f64 * 31.0);
            if game.can_place_tower(point) && game.place_tower(kinds[placed % kinds.len()], point) {
                placed += 1;
            }
        }
    }
    assert!(placed >= 30, "placed {placed} towers");
    let towers: Vec<Point> = game.runtime.towers.iter().map(|tower| tower.borrow().position()).collect();
    for (index, point) in towers.iter().enumerate() {
        game.select_tower_at(*point);
        for _ in 0..index % 7 {
            game.upgrade_selected_tower();
        }
    }
    game.runtime.selected_tower = None;
}

#[test]
fn busy_battle_draws_every_view() {
    rng::seed(7);
    let mut game = Game::new(GameMode::Desktop, Box::new(MemoryProgressStorage::default()));
    game.unlock_all_levels_for_debug();
    game.start_level_by_index(game.campaign_level_count() - 1);
    game.runtime.spawn_delay = 0.5;
    let profile = GameProfile::for_mode(GameMode::Desktop);
    let mut composer = FrameComposer::new(&profile, 1.0);
    composer.rig.resize(1280.0, 720.0);
    game.set_visible_field_bounds(composer.rig.field_bounds());
    fortify(&mut game);

    composer.sync_view_direction();
    let mut views = EntityViews::default();

    let mut seen = [0usize; crate::render_batches::INSTANCED_BATCHES];
    let mut max_monsters = 0;
    let mut peak_particles = 0;
    let mut now = 0.0;
    for step in 0..4800 {
        if game.state != GameState::Playing {
            break;
        }
        game.update_simulation(1.0 / 60.0);
        if step % 2 == 0 {
            now += 33.0;
            compose(&mut composer, &mut views, &game, now);
            for (index, batch) in composer.batches.instanced().iter().enumerate() {
                seen[index] = seen[index].max(batch.size());
                assert!(batch.used_data().iter().all(|value| value.is_finite()), "{} has non-finite data", batch.name);
            }
            assert!(composer.batches.glow.used_data().iter().all(|value| value.is_finite()));
            max_monsters = max_monsters.max(game.runtime.monsters.len());
            peak_particles = peak_particles.max(composer.fx.active_particles());
            let uniforms = composer.frame_uniforms();
            assert!(uniforms.iter().all(|value| value.is_finite()));
        }
    }
    let available = game.current_level().expect("level").available_towers.clone();
    for (kind, parts) in [
        (TowerKind::Gun, &[Batch::GunHead, Batch::GunBarrel][..]),
        (TowerKind::Laser, &[Batch::LaserCrystal, Batch::LaserCradle]),
        (TowerKind::Missile, &[Batch::MissileLauncher, Batch::Missile]),
        (TowerKind::Slow, &[Batch::SlowCore, Batch::OrbNode]),
        (TowerKind::Drone, &[Batch::DronePad, Batch::DroneBody]),
        (TowerKind::Lightning, &[Batch::TeslaCoil]),
    ] {
        if available.contains(&kind) {
            for batch in parts {
                assert!(seen[*batch as usize] > 0, "{batch:?} never drew");
            }
        }
    }
    assert!(max_monsters > 5, "monsters spawned: {max_monsters}");
    for batch in [
        Batch::TowerBase,
        Batch::TowerRim,
        Batch::UpgradeRing,
        Batch::Pip,
        Batch::Shard,
        Batch::HealthBar,
        Batch::Ribbon,
        Batch::Decal,
        Batch::GroundGlow,
        Batch::RoadChevron,
        Batch::Portal,
        Batch::SpawnGate,
    ] {
        assert!(seen[batch as usize] > 0, "{batch:?} never drew");
    }
    let monster_batches = [
        Batch::PackmanJaw,
        Batch::SquareBody,
        Batch::TriangleBody,
        Batch::TankHull,
        Batch::RunnerBody,
        Batch::SplitterBody,
        Batch::BerserkerBody,
        Batch::BulwarkShell,
    ];
    assert!(monster_batches.iter().filter(|batch| seen[**batch as usize] > 0).count() >= 4);
    assert!(peak_particles > 0, "deaths and blasts spawn renderer fx");
}

#[test]
fn placement_hologram_and_selection_range_draw() {
    rng::seed(3);
    let mut game = Game::new(GameMode::Desktop, Box::new(MemoryProgressStorage::default()));
    game.start_level_by_index(0);
    let profile = GameProfile::for_mode(GameMode::Desktop);
    let mut composer = FrameComposer::new(&profile, 1.0);
    composer.rig.resize(1280.0, 720.0);
    game.set_visible_field_bounds(composer.rig.field_bounds());
    let mut views = EntityViews::default();

    game.start_tower_placement(TowerKind::Laser);
    game.set_pointer(Some(Point::new(400.0, 40.0)));
    compose(&mut composer, &mut views, &game, 16.0);
    assert_eq!(composer.batches[Batch::Range].size(), 1);
    assert_eq!(composer.batches[Batch::LaserCradle].size(), 1, "hologram tower");
    assert_eq!(composer.batches[Batch::Ribbon].size(), 2, "crosshair guides");
    assert_eq!(composer.batches[Batch::TowerBase].size(), 1);

    game.runtime.money = 1000;
    let spot = (0..20)
        .flat_map(|row| (0..30).map(move |column| Point::new(20.0 + column as f64 * 26.0, 20.0 + row as f64 * 21.0)))
        .find(|point| game.can_place_tower(*point))
        .expect("a free spot");
    assert!(game.place_tower(TowerKind::Gun, spot));
    // Placing selects the new tower.
    compose(&mut composer, &mut views, &game, 32.0);
    assert_eq!(composer.batches[Batch::Range].size(), 1, "selected tower range");
    assert_eq!(composer.batches[Batch::GunHead].size(), 1);
}

// ------------------------------------------------ board camera (scripts/check-runtime.mjs)

const FIELD: (f32, f32) = (800.0, 450.0);
const RECT: SurfaceRect = SurfaceRect { left: 0.0, top: 0.0, width: 800.0, height: 450.0 };

fn frames_field(rig: &CameraRig) -> bool {
    [(0.0, 0.0), (FIELD.0, 0.0), (0.0, FIELD.1), (FIELD.0, FIELD.1)].iter().all(|&(x, y)| {
        let screen = rig.project_to_viewport(x, 0.0, y);
        (0.0..=800.0).contains(&screen.x) && (0.0..=450.0).contains(&screen.y)
    })
}

fn picks_back(rig: &CameraRig) -> bool {
    let screen = rig.project_to_viewport(600.0, 0.0, 120.0);
    rig.client_to_field(screen.x, screen.y, &RECT)
        .is_some_and(|picked| (picked.x - 600.0).hypot(picked.y - 120.0) < 1e-3)
}

fn field_at(rig: &CameraRig, x: f64, y: f64) -> Option<Point> {
    rig.client_to_field(x, y, &RECT)
}

fn near_point(a: Option<Point>, b: Option<Point>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if (a.x - b.x).hypot(a.y - b.y) < 1e-3)
}

#[test]
fn board_view_controls_match_the_runtime_checks() {
    let mut rig = create_board_camera_rig(FIELD.0, FIELD.1);
    rig.resize(800.0, 450.0);
    let initial_bounds: FieldBounds = rig.field_bounds();
    let initial_tilt = rig.tilt();
    let same_bounds = |rig: &CameraRig| rig.field_bounds() == initial_bounds;

    for (delta, direction) in [(-1.0, "straight down"), (2.0, "toward the horizon")] {
        let moved = rig.tilt_by(delta);
        assert!(moved && !rig.tilt_by(delta) && rig.tilt() != initial_tilt, "Board tilt clamps {direction}");
        assert!(
            frames_field(&rig) && picks_back(&rig) && same_bounds(&rig),
            "Board tilt {direction} keeps the field framed, picking exact, and gameplay bounds fixed"
        );
        rig.resize(800.0, 450.0);
        assert!(same_bounds(&rig) && frames_field(&rig), "Board tilt {direction} survives a resize");
    }

    assert!(
        rig.reset_view() && !rig.reset_view() && rig.tilt() == initial_tilt && frames_field(&rig),
        "Board view reset restores the default framing"
    );
    assert!(!rig.pan_between(400.0, 225.0, 300.0, 200.0, &RECT), "Board pan is locked while the whole field is framed");
    let cursor_ground = field_at(&rig, 620.0, 140.0);
    assert!(
        rig.zoom_at(2.0, 620.0, 140.0, &RECT) && near_point(field_at(&rig, 620.0, 140.0), cursor_ground),
        "Board zoom keeps the ground under the cursor in place"
    );
    let grabbed = field_at(&rig, 300.0, 200.0);
    assert!(
        rig.pan_between(300.0, 200.0, 360.0, 250.0, &RECT) && near_point(field_at(&rig, 360.0, 250.0), grabbed),
        "Board pan keeps the grabbed ground under the cursor"
    );
    assert!(
        picks_back(&rig) && same_bounds(&rig),
        "Zoomed and panned board keeps picking exact and gameplay bounds fixed"
    );
    rig.pan_between(400.0, 225.0, 5400.0, 5225.0, &RECT);
    let center = field_at(&rig, 400.0, 225.0).expect("center on the ground");
    assert!(
        !rig.pan_between(400.0, 225.0, 500.0, 300.0, &RECT)
            && center.x > 0.0
            && center.x < FIELD.0 as f64 / 2.0
            && center.y > 0.0
            && center.y < FIELD.1 as f64 / 2.0,
        "Board pan stops at the field edge"
    );
    assert!(
        rig.zoom_at(100.0, 400.0, 225.0, &RECT) && rig.zoom_factor() == 4.0 && !rig.zoom_at(2.0, 400.0, 225.0, &RECT),
        "Board zoom clamps at 4x"
    );
    rig.zoom_at(1e-3, 400.0, 225.0, &RECT);
    assert!(
        rig.zoom_factor() == 1.0 && frames_field(&rig),
        "Zooming fully out re-frames the whole field and drops the pan"
    );
    rig.resize(800.0, 450.0);
    assert!(same_bounds(&rig) && frames_field(&rig), "Board view survives a resize");
}
