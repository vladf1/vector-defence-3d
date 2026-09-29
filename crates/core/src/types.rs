//! Shared simulation types. String forms match the TypeScript shell's `src/types.ts`.

/// An sRGB color as `0xRRGGBB`, the same values the original authored as `#rrggbb`.
pub type Color = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState {
    Menu,
    Playing,
    Paused,
    DefeatPending,
    Won,
    Lost,
    CampaignWon,
}

impl GameState {
    pub fn as_str(self) -> &'static str {
        match self {
            GameState::Menu => "menu",
            GameState::Playing => "playing",
            GameState::Paused => "paused",
            GameState::DefeatPending => "defeat-pending",
            GameState::Won => "won",
            GameState::Lost => "lost",
            GameState::CampaignWon => "campaign-won",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MonsterKind {
    PackMan,
    Square,
    Triangle,
    Tank,
    Runner,
    Splitter,
    Berserker,
    Bulwark,
}

impl MonsterKind {
    pub const ALL: [MonsterKind; 8] = [
        MonsterKind::PackMan,
        MonsterKind::Square,
        MonsterKind::Triangle,
        MonsterKind::Tank,
        MonsterKind::Runner,
        MonsterKind::Splitter,
        MonsterKind::Berserker,
        MonsterKind::Bulwark,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            MonsterKind::PackMan => "packman",
            MonsterKind::Square => "square",
            MonsterKind::Triangle => "triangle",
            MonsterKind::Tank => "tank",
            MonsterKind::Runner => "runner",
            MonsterKind::Splitter => "splitter",
            MonsterKind::Berserker => "berserker",
            MonsterKind::Bulwark => "bulwark",
        }
    }

    pub fn parse(value: &str) -> Option<MonsterKind> {
        MonsterKind::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

/// Declaration order is the toolbar order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TowerKind {
    Gun,
    Laser,
    Missile,
    Slow,
    Drone,
    Lightning,
}

impl TowerKind {
    pub const ALL: [TowerKind; 6] =
        [TowerKind::Gun, TowerKind::Laser, TowerKind::Missile, TowerKind::Slow, TowerKind::Drone, TowerKind::Lightning];

    pub fn as_str(self) -> &'static str {
        match self {
            TowerKind::Gun => "gun",
            TowerKind::Laser => "laser",
            TowerKind::Missile => "missile",
            TowerKind::Slow => "slow",
            TowerKind::Drone => "drone",
            TowerKind::Lightning => "lightning",
        }
    }

    pub fn parse(value: &str) -> Option<TowerKind> {
        TowerKind::ALL.into_iter().find(|kind| kind.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalAction {
    Resume,
    PlayUnlocked,
    RestartCampaign,
    NextLevel,
    Replay,
    CampaignMap,
}

impl ModalAction {
    pub const ALL: [ModalAction; 6] = [
        ModalAction::Resume,
        ModalAction::PlayUnlocked,
        ModalAction::RestartCampaign,
        ModalAction::NextLevel,
        ModalAction::Replay,
        ModalAction::CampaignMap,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ModalAction::Resume => "resume",
            ModalAction::PlayUnlocked => "play-unlocked",
            ModalAction::RestartCampaign => "restart-campaign",
            ModalAction::NextLevel => "next-level",
            ModalAction::Replay => "replay",
            ModalAction::CampaignMap => "campaign-map",
        }
    }

    pub fn parse(value: &str) -> Option<ModalAction> {
        ModalAction::ALL.into_iter().find(|action| action.as_str() == value)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Point {
        Point { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldBounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

#[derive(Clone, Debug)]
pub struct WaveData {
    pub count: u32,
    pub monster_sequence: Vec<MonsterKind>,
    pub spawn_interval_min: f64,
    pub spawn_interval_max: f64,
    pub build_time: f64,
    pub reward: i32,
}

/// An authored route after mobile overrides (generated from `game-levels.json`).
#[derive(Debug)]
pub struct RouteData {
    pub name: &'static str,
    pub subtitle: Option<&'static str>,
    pub allow_escape: i32,
    pub starting_money: i32,
    pub wave_count: Option<usize>,
    pub initial_build_time: Option<f64>,
    pub monster_sequence: &'static [MonsterKind],
    pub available_towers: &'static [TowerKind],
    pub points: &'static [Point],
}

/// A playable campaign level, expanded from a route by `campaign::create_campaign_levels`.
#[derive(Clone, Debug)]
pub struct LevelData {
    pub id: String,
    pub level_number: u32,
    pub name: &'static str,
    pub subtitle: &'static str,
    pub allow_escape: i32,
    pub starting_money: i32,
    pub available_towers: Vec<TowerKind>,
    pub points: Vec<Point>,
    pub waves: Vec<WaveData>,
    pub monster_count: u32,
}
