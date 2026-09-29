//! Desktop/mobile logical dimensions, movement/range scales, and placement geometry.
//! Profile selection (media queries) stays in the page shell's `src/game-profile.ts`.
use crate::constants::{
    FIELD_HEIGHT, FIELD_WIDTH, MAX_TOWER_LEVEL, ROAD_TURN_RADIUS, ROAD_WIDTH, ROUTE_CURVE_SAMPLE_STEP, TOWER_RADIUS,
    TOWER_ROAD_EDGE_OVERLAP_ALLOWANCE, TOWER_UPGRADE_RING_GROWTH, TOWER_UPGRADE_RING_OFFSET,
};
use crate::placement::PlacementGeometry;
use crate::types::FieldBounds;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameMode {
    Desktop,
    Mobile,
}

#[derive(Clone, Debug)]
pub struct GameProfile {
    pub mode: GameMode,
    pub field_width: f64,
    pub field_height: f64,
    pub tower_radius: f64,
    pub tower_selection_padding: f64,
    pub tower_range_scale: f64,
    pub monster_speed_scale: f64,
    pub road_turn_radius: f64,
    pub road_width: f64,
    pub route_curve_sample_step: f64,
    pub placement: PlacementGeometry,
    /// Canvas tower actions (upgrade/lock buttons) are drawn beside the selected tower.
    pub draw_canvas_tower_actions: bool,
}

struct ProfileOptions {
    mode: GameMode,
    field_width: f64,
    field_height: f64,
    tower_radius: f64,
    tower_selection_padding: f64,
    tower_range_scale: f64,
    monster_speed_scale: f64,
    min_distance_to_other_towers: f64,
    road_turn_radius: f64,
    road_width: f64,
    route_curve_sample_step: f64,
    draw_canvas_tower_actions: bool,
}

fn create_profile(options: ProfileOptions) -> GameProfile {
    let max_tower_body_radius =
        options.tower_radius + TOWER_UPGRADE_RING_OFFSET + MAX_TOWER_LEVEL as f64 * TOWER_UPGRADE_RING_GROWTH;
    let placement = PlacementGeometry {
        bounds: FieldBounds { min_x: 0.0, min_y: 0.0, max_x: options.field_width, max_y: options.field_height },
        tower_radius: options.tower_radius,
        tower_selection_padding: options.tower_selection_padding,
        min_distance_to_other_towers: options.min_distance_to_other_towers,
        min_distance_to_road: options.road_width / 2.0 + max_tower_body_radius - TOWER_ROAD_EDGE_OVERLAP_ALLOWANCE,
    };
    GameProfile {
        mode: options.mode,
        field_width: options.field_width,
        field_height: options.field_height,
        tower_radius: options.tower_radius,
        tower_selection_padding: options.tower_selection_padding,
        tower_range_scale: options.tower_range_scale,
        monster_speed_scale: options.monster_speed_scale,
        road_turn_radius: options.road_turn_radius,
        road_width: options.road_width,
        route_curve_sample_step: options.route_curve_sample_step,
        placement,
        draw_canvas_tower_actions: options.draw_canvas_tower_actions,
    }
}

impl GameProfile {
    pub fn for_mode(mode: GameMode) -> GameProfile {
        match mode {
            GameMode::Desktop => create_profile(ProfileOptions {
                mode,
                field_width: FIELD_WIDTH,
                field_height: FIELD_HEIGHT,
                tower_radius: TOWER_RADIUS,
                tower_range_scale: 1.0,
                monster_speed_scale: 1.0,
                min_distance_to_other_towers: 32.0,
                tower_selection_padding: 6.0,
                road_turn_radius: ROAD_TURN_RADIUS,
                road_width: ROAD_WIDTH,
                route_curve_sample_step: ROUTE_CURVE_SAMPLE_STEP,
                draw_canvas_tower_actions: true,
            }),
            GameMode::Mobile => create_profile(ProfileOptions {
                mode,
                field_width: 390.0,
                field_height: 560.0,
                tower_radius: TOWER_RADIUS,
                tower_range_scale: 0.9,
                monster_speed_scale: 0.9,
                min_distance_to_other_towers: 27.0,
                tower_selection_padding: 12.0,
                road_turn_radius: 34.0,
                road_width: 25.0,
                route_curve_sample_step: 4.0,
                draw_canvas_tower_actions: false,
            }),
        }
    }
}
