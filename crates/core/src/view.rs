//! HUD and modal view models as JSON that `JSON.parse` turns into exactly the TS `HudSnapshot`
//! and `ModalView` shapes (`src/types.ts`): banner text, labels, and formatting live here.
use crate::entities::towers::registry::{TOWER_INFOS, tower_info};
use crate::game::Game;
use crate::json::{ObjectWriter, js_to_fixed, write_array, write_string};
use crate::profile::GameMode;
use crate::types::{GameState, ModalAction};
use crate::utils::{format_money, js_round};

/// Frame timing sampled by the page for the nerd-stats panel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimeHudStats {
    pub fps: f64,
    pub frame_time_ms: f64,
    pub update_time_ms: f64,
    pub draw_time_ms: f64,
}

const DEFAULT_SELECTION_SUMMARY: &str = "Select a tower to view upgrades, range, and sell value.";

fn format_timing_ms(value: f64) -> String {
    format!("{} ms", js_to_fixed(value, 3))
}

fn write_affordable_towers(out: &mut String, money: i32) {
    let mut object = ObjectWriter::new(out);
    for info in &TOWER_INFOS {
        object.boolean(info.kind.as_str(), money >= info.base_cost);
    }
    object.finish();
}

/// The banner line: the pre-wave countdown, then timed banners, then state fallbacks.
pub fn create_banner_text(game: &Game) -> String {
    let active_wave = game.runtime.active_wave();
    if game.state == GameState::Playing && active_wave.is_some() && game.runtime.spawn_delay > 0.0 {
        return format!("NEXT WAVE IN {}", game.runtime.spawn_delay.ceil() as i64);
    }
    if matches!(game.state, GameState::Won | GameState::CampaignWon | GameState::Lost) {
        return String::new();
    }
    if game.banner_timer > 0.0 {
        return game.banner_text.clone();
    }
    match game.state {
        GameState::Menu => "Awaiting orders".to_string(),
        GameState::Paused => "Paused".to_string(),
        GameState::DefeatPending => "Base breached".to_string(),
        _ => String::new(),
    }
}

/// `createHudSnapshot(game, runtimeStats)` as JSON.
pub fn create_hud_json(game: &Game, stats: &RuntimeHudStats) -> String {
    let runtime = &game.runtime;
    let current_level = game.current_level();
    let selected = runtime.selected_tower.as_ref().map(|tower| tower.borrow());
    let active_wave = runtime.active_wave();
    let battle_actions_disabled = !game.can_perform_battle_action();
    let banner = create_banner_text(game);
    let mobile = game.profile.mode == GameMode::Mobile;

    let mut selection_name = String::new();
    let mut selection_summary = DEFAULT_SELECTION_SUMMARY.to_string();
    let can_upgrade = selected.as_ref().is_some_and(|tower| tower.can_upgrade());
    let upgrade_value = match &selected {
        Some(tower) if tower.can_upgrade() => format_money(tower.upgrade_cost() as f64),
        _ => "Max".to_string(),
    };
    let sell_value = selected.as_ref().map_or("Sell".to_string(), |tower| format_money(tower.resale_value() as f64));

    if let Some(tower) = &selected {
        let details = format!("Level {} · Range {}", tower.level + 1, js_round(tower.range) as i64);
        let label = tower_info(tower.kind).label;
        selection_name = if mobile { format!("{label} Tower") } else { format!("{label} Tower · {details}") };
        selection_summary = if mobile { details } else { String::new() };
    } else if let Some(placing_tower) = runtime.placing_tower {
        let info = tower_info(placing_tower);
        selection_name = format!("Placing {} Tower", info.label);
        selection_summary = if mobile {
            format!("Tap field to build · {}", format_money(info.base_cost as f64))
        } else {
            info.summary.to_string()
        };
    }

    let shots_tracked = runtime.projectiles.len() + runtime.missiles.len() + runtime.drones.len();
    let effects_tracked = runtime.particles.len() + runtime.links.len();
    let tracked_objects = runtime.towers.len() + runtime.monsters.len() + shots_tracked + effects_tracked;
    let upgrade_unaffordable =
        selected.as_ref().is_some_and(|tower| tower.can_upgrade() && runtime.money < tower.upgrade_cost());
    let upgrade_disabled = match &selected {
        None => true,
        Some(tower) => !tower.can_upgrade() || runtime.money < tower.upgrade_cost() || battle_actions_disabled,
    };
    let is_laser = selected.as_ref().is_some_and(|tower| tower.is_laser());
    let laser_locked = selected.as_ref().is_some_and(|tower| tower.direction_locked());

    let mut out = String::with_capacity(1024);
    let mut object = ObjectWriter::new(&mut out);
    if let Some(level) = current_level {
        object.integer("levelNumber", level.level_number as i64);
    }
    object.integer("money", runtime.money as i64);
    if active_wave.is_some() {
        object.integer("waveCurrent", runtime.current_wave_index as i64 + 1);
    }
    object.integer("waveTotal", runtime.wave_total() as i64);
    if let Some(wave) = active_wave {
        object.integer("waveMonstersSpawned", runtime.wave_spawned_monsters.min(wave.count) as i64);
        object.integer("waveMonsterTotal", wave.count as i64);
    }
    object
        .string("banner", &banner)
        .string("selectionName", &selection_name)
        .string("selectionSummary", &selection_summary)
        .string("upgradeLabel", &if can_upgrade { format!("Upgrade - {upgrade_value}") } else { "Max".to_string() })
        .string("upgradeValue", &upgrade_value)
        .string("sellLabel", &if selected.is_some() { format!("Sell - {sell_value}") } else { "Sell".to_string() })
        .string("sellValue", &sell_value)
        .boolean("upgradeDisabled", upgrade_disabled)
        .boolean("upgradeUnaffordable", upgrade_unaffordable)
        .boolean("hasSelectedTower", selected.is_some())
        .boolean("hasLaserLockAction", is_laser)
        .boolean("laserLocked", is_laser && laser_locked)
        .boolean("laserLockDisabled", !is_laser || battle_actions_disabled)
        .boolean("sellDisabled", selected.is_none() || battle_actions_disabled)
        .boolean("cancelBuildDisabled", runtime.placing_tower.is_none() || battle_actions_disabled)
        .boolean("canTogglePause", matches!(game.state, GameState::Playing | GameState::Paused))
        .boolean("showStatusHud", current_level.is_some() && game.state != GameState::Menu)
        .boolean(
            "canSkipBreak",
            game.state == GameState::Playing
                && active_wave.is_some()
                && runtime.spawn_delay > 0.0
                && !battle_actions_disabled,
        )
        .boolean("paused", game.state == GameState::Paused);
    if let Some(placing_tower) = runtime.placing_tower {
        object.string("placingTower", placing_tower.as_str());
    }
    object.boolean("towerButtonsDisabled", battle_actions_disabled);

    let mut available_towers = String::new();
    let available = current_level.map_or(&[][..], |level| &level.available_towers[..]);
    write_array(&mut available_towers, available, |out, kind| write_string(out, kind.as_str()));
    object.raw("availableTowers", &available_towers);

    let mut affordable_towers = String::new();
    write_affordable_towers(&mut affordable_towers, runtime.money);
    object.raw("affordableTowers", &affordable_towers);

    let mut nerd_stats = String::new();
    let mut stats_object = ObjectWriter::new(&mut nerd_stats);
    stats_object
        .string("fps", &(js_round(stats.fps).max(0.0) as i64).to_string())
        .string("frameTime", &format!("{} ms", js_to_fixed(stats.frame_time_ms, 1)))
        .string("updateTime", &format_timing_ms(stats.update_time_ms))
        .string("drawTime", &format_timing_ms(stats.draw_time_ms))
        .string("trackedObjects", &tracked_objects.to_string())
        .string("towers", &runtime.towers.len().to_string())
        .string("hostiles", &runtime.monsters.len().to_string())
        .string("shots", &shots_tracked.to_string())
        .string("effects", &effects_tracked.to_string());
    stats_object.finish();
    object.raw("nerdStats", &nerd_stats);
    object.finish();
    out
}

fn write_actions(out: &mut String, actions: &[(ModalAction, String)]) {
    write_array(out, actions, |out, (action, label)| {
        let mut object = ObjectWriter::new(out);
        object.string("action", action.as_str()).string("label", label);
        object.finish();
    });
}

fn format_star_count(stars: u32) -> String {
    format!("{stars} star{}", if stars == 1 { "" } else { "s" })
}

fn award_title(stars: u32) -> &'static str {
    match stars {
        3 => "Perfect route",
        2 => "Strong clear",
        _ => "Route secured",
    }
}

fn award_copy(stars: u32, best_stars: u32) -> String {
    if best_stars > stars {
        return format!("Best clear remains {}.", format_star_count(best_stars));
    }
    match stars {
        3 => "No leaks. Full control.".to_string(),
        2 => "Cleared with escape room to spare.".to_string(),
        _ => "Replay for a cleaner defense.".to_string(),
    }
}

fn level_stars(game: &Game, index: usize) -> u32 {
    game.level_stars.get(index).copied().unwrap_or(0)
}

fn write_star_award(out: &mut String, game: &Game) {
    let best_stars =
        if game.current_level_index >= 0 { level_stars(game, game.current_level_index as usize) } else { 0 };
    let stars = game.last_awarded_stars;
    let mut object = ObjectWriter::new(out);
    object
        .integer("stars", stars as i64)
        .string("title", award_title(stars))
        .string("description", &award_copy(stars, best_stars))
        .string("label", &format!("{} awarded", format_star_count(stars)))
        .boolean("perfect", stars == 3);
    object.finish();
}

fn write_level_cards(out: &mut String, game: &Game) {
    write_array(out, game.levels.iter().enumerate(), |out, (index, level)| {
        let unlocked =
            game.debug_all_levels_unlocked || game.campaign_cleared || index <= game.highest_unlocked_level_index;
        let cleared = game.campaign_cleared || index < game.highest_unlocked_level_index;
        let current = game.current_level_index == index as i32 && game.current_level().is_some();
        let next_index = game.highest_unlocked_level_index.min(game.campaign_level_count().saturating_sub(1));
        let status = if !unlocked {
            "Locked"
        } else if cleared {
            "Cleared"
        } else if index == next_index {
            "Next"
        } else {
            "Ready"
        };
        let stars = level_stars(game, index);
        let description = level.subtitle.strip_suffix('.').unwrap_or(level.subtitle);
        let mut object = ObjectWriter::new(out);
        object
            .integer("index", index as i64)
            .boolean("unlocked", unlocked)
            .boolean("cleared", cleared)
            .boolean("current", current)
            .integer("stars", stars as i64)
            .string("status", status)
            .string("title", &format!("{} - {}", level.level_number, level.name))
            .string("description", description)
            .string("summary", &format!("{} waves · {} enemies", level.waves.len(), level.monster_count))
            .string("starsLabel", &format!("{} best clear", format_star_count(stars)));
        object.finish();
    });
}

/// `createModalView(game)` as JSON, or `None` when no modal is shown.
pub fn create_modal_json(game: &Game) -> Option<String> {
    let mut out = String::with_capacity(512);
    match game.state {
        GameState::Menu => {
            let can_resume = game.menu_return_state.is_some() && game.current_level().is_some();
            let mut actions = vec![if can_resume {
                (ModalAction::Resume, "Resume Battle".to_string())
            } else if game.campaign_cleared {
                (ModalAction::PlayUnlocked, "Replay Campaign".to_string())
            } else {
                (ModalAction::PlayUnlocked, "Play Next".to_string())
            }];
            if game.highest_unlocked_level_index > 0 || game.campaign_cleared {
                actions.push((ModalAction::RestartCampaign, "Restart Campaign".to_string()));
            }
            let mut actions_json = String::new();
            write_actions(&mut actions_json, &actions);
            let mut cards_json = String::new();
            write_level_cards(&mut cards_json, game);
            let mut object = ObjectWriter::new(&mut out);
            object
                .string("title", "Campaign Map")
                .string(
                    "description",
                    &format!("{} campaign battles. Clear one route to unlock the next", game.campaign_level_count()),
                )
                .raw("actions", &actions_json)
                .raw("levelCards", &cards_json);
            object.finish();
        }
        GameState::Won => {
            let level_number = game.current_level().map(|level| level.level_number);
            let actions = [
                (ModalAction::NextLevel, format!("Continue to Level {}", level_number.unwrap_or(0) + 1)),
                (ModalAction::CampaignMap, "Campaign Map".to_string()),
                (ModalAction::Replay, "Replay".to_string()),
            ];
            let description = format!(
                "Level {} secured. Next route unlocked!",
                level_number.map_or("?".to_string(), |number| number.to_string())
            );
            write_sheet(&mut out, game, "Level Clear", &description, true, &actions);
        }
        GameState::CampaignWon => {
            let actions = [
                (ModalAction::RestartCampaign, "Restart Campaign".to_string()),
                (ModalAction::CampaignMap, "Campaign Map".to_string()),
                (ModalAction::Replay, "Replay".to_string()),
            ];
            let description = format!("All {} campaign levels are secure", game.campaign_level_count());
            write_sheet(&mut out, game, "You Won the Campaign", &description, true, &actions);
        }
        GameState::Lost => {
            let actions = [
                (ModalAction::Replay, "Try Again".to_string()),
                (ModalAction::CampaignMap, "Campaign Map".to_string()),
            ];
            write_sheet(
                &mut out,
                game,
                "Defeat",
                "Too many enemies reached the exit. Better luck next time!",
                false,
                &actions,
            );
        }
        _ => return None,
    }
    Some(out)
}

fn write_sheet(
    out: &mut String,
    game: &Game,
    title: &str,
    description: &str,
    star_award: bool,
    actions: &[(ModalAction, String)],
) {
    let mut actions_json = String::new();
    write_actions(&mut actions_json, actions);
    let mut object = ObjectWriter::new(out);
    object.string("title", title).string("description", description).boolean("sheet", true);
    if star_award {
        let mut award_json = String::new();
        write_star_award(&mut award_json, game);
        object.raw("starAward", &award_json);
    }
    object.raw("actions", &actions_json);
    object.finish();
}

/// The modal JSON, or `null` when no modal is shown.
pub fn create_modal_json_or_null(game: &Game) -> String {
    create_modal_json(game).unwrap_or_else(|| "null".to_string())
}

/// Toolbar metadata (`TowerCatalogEntry[]`) in toolbar order.
pub fn tower_catalog_json() -> String {
    let mut out = String::new();
    write_array(&mut out, TOWER_INFOS.iter(), |out, info| {
        let mut shortcuts = String::new();
        write_array(&mut shortcuts, info.shortcuts.iter(), |out, shortcut| write_string(out, shortcut));
        let mut object = ObjectWriter::new(out);
        object
            .string("kind", info.kind.as_str())
            .string("label", info.label)
            .string("summary", info.summary)
            .integer("baseCost", info.base_cost as i64)
            .raw("shortcuts", &shortcuts);
        object.finish();
    });
    out
}
