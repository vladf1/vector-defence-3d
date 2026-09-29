//! Tower metadata, keyboard shortcuts, and toolbar order (`TOWER_CLASSES` in the TS).
use crate::types::TowerKind;

#[derive(Debug)]
pub struct TowerInfo {
    pub kind: TowerKind,
    pub label: &'static str,
    pub summary: &'static str,
    pub base_cost: i32,
    pub base_range: f64,
    pub shortcuts: &'static [&'static str],
}

/// Registration order is the toolbar order (the declaration order of `TowerKind::ALL`).
pub static TOWER_INFOS: [TowerInfo; 6] = [
    TowerInfo {
        kind: TowerKind::Gun,
        label: "Gun",
        summary: "Fast, cheap, accurate lead shots.",
        base_cost: 2,
        base_range: 60.0,
        shortcuts: &["1", "g"],
    },
    TowerInfo {
        kind: TowerKind::Laser,
        label: "Laser",
        summary: "Piercing beam that melts lines of enemies.",
        base_cost: 3,
        base_range: 100.0,
        shortcuts: &["2", "z"],
    },
    TowerInfo {
        kind: TowerKind::Missile,
        label: "Missile",
        summary: "Slow launcher with splash damage.",
        base_cost: 5,
        base_range: 150.0,
        shortcuts: &["3", "r"],
    },
    TowerInfo {
        kind: TowerKind::Slow,
        label: "Slow",
        summary: "Freezes clusters so the rest can clean up.",
        base_cost: 3,
        base_range: 70.0,
        shortcuts: &["4", "s"],
    },
    TowerInfo {
        kind: TowerKind::Drone,
        label: "Drone",
        summary: "Launches one autonomous hunter drone on a long cooldown.",
        base_cost: 5,
        base_range: 115.0,
        shortcuts: &["5", "d"],
    },
    TowerInfo {
        kind: TowerKind::Lightning,
        label: "Lightning",
        summary: "Chains shocks that damage and heavily slow monsters.",
        base_cost: 5,
        base_range: 74.0,
        shortcuts: &["5", "e"],
    },
];

pub fn tower_info(kind: TowerKind) -> &'static TowerInfo {
    &TOWER_INFOS[kind as usize]
}

/// The first tower (in toolbar order) available on the level whose shortcuts include `key`.
pub fn find_tower_shortcut(key: &str, available_towers: &[TowerKind]) -> Option<TowerKind> {
    let normalized_key = key.to_ascii_lowercase();
    TOWER_INFOS
        .iter()
        .find(|info| available_towers.contains(&info.kind) && info.shortcuts.contains(&normalized_key.as_str()))
        .map(|info| info.kind)
}

/// Two available towers sharing a shortcut (`normalizeAvailableTowers` rejects such levels).
pub fn find_shortcut_conflict(available_towers: &[TowerKind]) -> Option<(&'static str, TowerKind, TowerKind)> {
    let mut seen: Vec<(&'static str, TowerKind)> = Vec::new();
    for &kind in available_towers {
        for &shortcut in tower_info(kind).shortcuts {
            if let Some(&(_, previous)) = seen.iter().find(|(key, _)| *key == shortcut) {
                return Some((shortcut, previous, kind));
            }
            seen.push((shortcut, kind));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaign::create_levels;
    use crate::profile::GameMode;

    #[test]
    fn registry_is_in_toolbar_order() {
        for (index, info) in TOWER_INFOS.iter().enumerate() {
            assert_eq!(info.kind, TowerKind::ALL[index]);
            assert_eq!(tower_info(info.kind).kind, info.kind);
        }
    }

    #[test]
    fn shortcuts_resolve_per_level() {
        assert_eq!(find_tower_shortcut("G", &[TowerKind::Gun]), Some(TowerKind::Gun));
        assert_eq!(find_tower_shortcut("5", &[TowerKind::Gun, TowerKind::Lightning]), Some(TowerKind::Lightning));
        assert_eq!(find_tower_shortcut("5", &[TowerKind::Gun]), None);
        assert_eq!(
            find_shortcut_conflict(&[TowerKind::Drone, TowerKind::Lightning]),
            Some(("5", TowerKind::Drone, TowerKind::Lightning))
        );
    }

    #[test]
    fn campaign_levels_have_no_shortcut_conflicts() {
        for mode in [GameMode::Desktop, GameMode::Mobile] {
            for level in create_levels(mode) {
                assert_eq!(find_shortcut_conflict(&level.available_towers), None, "{}", level.name);
            }
        }
    }
}
