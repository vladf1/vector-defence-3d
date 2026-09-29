//! Procedural low-poly models. Monster models are built at unit radius and scaled per
//! instance by the monster's gameplay radius; tower models use field units (tower radius 12).
//! Local +X is "forward" (2D angle 0) and +Z maps to field +Y.
use std::f32::consts::PI;

use crate::geometry_kit::{
    Mesh, P2, Part, bar, bipyramid, box3, circle, cone, cylinder, extrude_outline, merge, octahedron, outline_trim, p2,
    place, recompute_flat_normals, scale_outline, solid, sphere, torus, transform, translate,
};
use crate::math::{Vec3, vec3};

const HALF_TURN: f32 = PI;
const QUARTER_TURN: f32 = PI / 2.0;
const FULL_TURN: f32 = PI * 2.0;

fn glow_trim(outline: &[P2], y: f32, thickness: f32, height: f32, glow: f32) -> Vec<Part> {
    outline_trim(outline, y, thickness, height).into_iter().map(|primitive| solid(primitive, glow, None)).collect()
}

/// A glowing bar from `from` to `to`: a lit ridge line along a faceted body's edge.
fn glow_ridge(from: Vec3, to: Vec3, thickness: f32, glow: f32) -> Part {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let dz = to.z - from.z;
    let horizontal = dx.hypot(dz);
    solid(
        box3(horizontal.hypot(dy) + thickness, thickness, thickness),
        glow,
        // Rz lifts the bar's X axis to the ridge's slope, then Ry turns it to the ridge's heading.
        Some(&place(
            from.x + dx / 2.0,
            from.y + dy / 2.0,
            from.z + dz / 2.0,
            0.0,
            -dz.atan2(dx),
            dy.atan2(horizontal),
            1.0,
            1.0,
            1.0,
        )),
    )
}

fn cylinder_along_x(radius: f32, length: f32, segments: u32, glow: f32, start_x: f32, y: f32) -> Part {
    solid(
        cylinder(radius, radius, length, segments),
        glow,
        Some(&place(start_x + length / 2.0, y, 0.0, 0.0, 0.0, -QUARTER_TURN, 1.0, 1.0, 1.0)),
    )
}

// ---------------------------------------------------------------- monsters

const PACKMAN_TRIM_SEGMENTS: u32 = 16;
const PACKMAN_TRIM_RADIUS: f32 = 1.02;
const PACKMAN_TRIM_THICKNESS: f32 = 0.13;

pub fn create_pack_man_jaw() -> Part {
    // The z <= 0 hemisphere (field "up" side) with its cut face capped; the second jaw is
    // this mesh flipped about X, which hides its eye underneath like the 2D single eye.
    let mut parts = vec![
        solid(sphere(1.0, 22, 14, HALF_TURN, HALF_TURN), 0.0, None),
        solid(circle(1.0, 22), 0.32, None),
        solid(sphere(0.17, 10, 8, 0.0, FULL_TURN), 1.0, Some(&translate(0.18, 0.8, -0.44))),
    ];
    // The original's outline from above: the jaw's half of the equator, plus the mouth edge
    // from the center to the front (the back half would show between the jaws).
    let thickness = PACKMAN_TRIM_THICKNESS;
    for segment in 0..PACKMAN_TRIM_SEGMENTS {
        let start = segment as f32 / PACKMAN_TRIM_SEGMENTS as f32 * PI;
        let end = (segment + 1) as f32 / PACKMAN_TRIM_SEGMENTS as f32 * PI;
        let start_x = start.cos() * PACKMAN_TRIM_RADIUS;
        let start_z = -start.sin() * PACKMAN_TRIM_RADIUS;
        let end_x = end.cos() * PACKMAN_TRIM_RADIUS;
        let end_z = -end.sin() * PACKMAN_TRIM_RADIUS;
        let length = (end_x - start_x).hypot(end_z - start_z);
        let angle = (end_z - start_z).atan2(end_x - start_x);
        let trim =
            bar((start_x + end_x) / 2.0, 0.0, (start_z + end_z) / 2.0, angle, length + thickness, thickness, thickness);
        parts.push(solid(trim, 1.0, None));
    }
    let mouth = bar(PACKMAN_TRIM_RADIUS / 2.0, 0.0, 0.0, 0.0, PACKMAN_TRIM_RADIUS + thickness, thickness, thickness);
    parts.push(solid(mouth, 1.0, None));
    merge(parts)
}

pub fn create_square_body() -> Part {
    let mut parts = vec![solid(box3(2.0, 2.0, 2.0), 0.0, None)];
    let edge = 2.12;
    let thickness = 0.16;
    for a in [-1.0, 1.0] {
        for b in [-1.0, 1.0] {
            parts.push(solid(box3(edge, thickness, thickness), 1.0, Some(&translate(0.0, a, b))));
            parts.push(solid(box3(thickness, edge, thickness), 1.0, Some(&translate(a, 0.0, b))));
            parts.push(solid(box3(thickness, thickness, edge), 1.0, Some(&translate(a, b, 0.0))));
        }
    }
    merge(parts)
}

const TRIANGLE_OUTLINE: [P2; 3] = [p2(1.0, 0.0), p2(-1.0, -1.0), p2(-1.0, 1.0)];
const TRIANGLE_TOP_APEX: Vec3 = vec3(-0.3, 0.62, 0.0);

pub fn create_triangle_body() -> Part {
    let mut parts = vec![solid(bipyramid(&TRIANGLE_OUTLINE, TRIANGLE_TOP_APEX, vec3(-0.3, -0.34, 0.0)), 0.0, None)];
    parts.extend(glow_trim(&TRIANGLE_OUTLINE, 0.0, 0.11, 0.11, 1.0));
    // Glowing ridges from the peak to each corner, so the pyramid reads from straight above.
    parts.extend(
        TRIANGLE_OUTLINE.iter().map(|corner| glow_ridge(TRIANGLE_TOP_APEX, vec3(corner.x, 0.0, corner.y), 0.07, 1.0)),
    );
    parts.push(solid(sphere(0.13, 8, 6, 0.0, FULL_TURN), 1.0, Some(&translate(0.25, 0.28, 0.0))));
    merge(parts)
}

const TANK_HULL_TOP: f32 = 0.82;

pub fn create_tank_hull() -> Part {
    let hull_outline = [p2(-1.0, -0.72), p2(1.1, -0.72), p2(1.1, 0.72), p2(-1.0, 0.72)];
    let mut parts = vec![
        solid(box3(2.1, 0.5, 1.44), 0.0, Some(&translate(0.05, 0.57, 0.0))),
        solid(box3(1.7, 0.1, 1.1), 0.08, Some(&translate(0.0, TANK_HULL_TOP - 0.02, 0.0))),
    ];
    parts.extend(glow_trim(&hull_outline, TANK_HULL_TOP - 0.03, 0.07, 0.07, 1.0));
    for side in [-1.0, 1.0] {
        parts.push(solid(box3(2.26, 0.5, 0.34), 0.04, Some(&translate(0.05, 0.26, side * 0.86))));
        parts.push(solid(box3(2.2, 0.06, 0.06), 1.0, Some(&translate(0.05, 0.53, side * 1.03))));
        for tread in 0..7 {
            let x = -0.95 + tread as f32 * 0.33;
            parts.push(solid(box3(0.1, 0.52, 0.36), 0.12, Some(&translate(x, 0.26, side * 0.86))));
        }
    }
    merge(parts)
}

/// Turret mesh with its origin at the turret pivot; also used by the flying-turret debris.
pub fn create_tank_turret() -> Part {
    merge(vec![
        solid(cylinder(0.44, 0.5, 0.34, 16), 0.0, Some(&translate(0.0, 0.17, 0.0))),
        solid(torus(0.46, 0.035, 4, 24), 1.0, Some(&place(0.0, 0.33, 0.0, QUARTER_TURN, 0.0, 0.0, 1.0, 1.0, 1.0))),
        cylinder_along_x(0.085, 1.2, 8, 0.0, 0.4, 0.2),
        cylinder_along_x(0.12, 0.16, 8, 1.0, 1.5, 0.2),
    ])
}

const RUNNER_OUTLINE: [P2; 6] =
    [p2(1.8, 0.0), p2(0.28, -0.86), p2(-1.35, -0.58), p2(-0.92, 0.0), p2(-1.35, 0.58), p2(0.28, 0.86)];

pub fn create_runner_body() -> Part {
    let mut parts = vec![solid(extrude_outline(&RUNNER_OUTLINE, 0.32, 0.1), 0.0, None)];
    parts.extend(glow_trim(&scale_outline(&RUNNER_OUTLINE, 1.05), 0.26, 0.09, 0.09, 1.0));
    let canopy = place(0.35, 0.5, 0.0, 0.0, 0.0, 0.0, 0.55, 0.2, 0.28);
    parts.push(solid(sphere(1.0, 10, 8, 0.0, FULL_TURN), 1.0, Some(&canopy)));
    merge(parts)
}

fn splitter_outline() -> [P2; 6] {
    std::array::from_fn(|index| {
        let angle = PI / 3.0 * index as f32;
        let radius = if index % 2 == 0 { 1.15 } else { 0.72 };
        p2(angle.cos() * radius, angle.sin() * radius)
    })
}

pub fn create_splitter_body() -> Part {
    let outline = splitter_outline();
    let mut parts = vec![solid(bipyramid(&outline, vec3(0.0, 0.66, 0.0), vec3(0.0, -0.42, 0.0)), 0.0, None)];
    parts.extend(glow_trim(&outline, 0.0, 0.1, 0.1, 1.0));
    parts.push(solid(bar(-0.28, 0.42, 0.02, 0.35f32.atan2(0.55), 0.68, 0.07, 0.08), 1.0, None));
    parts.push(solid(bar(0.29, 0.42, 0.01, (-0.38f32).atan2(0.58), 0.7, 0.07, 0.08), 1.0, None));
    merge(parts)
}

const BERSERKER_OUTLINE: [P2; 8] = [
    p2(1.55, 0.0),
    p2(0.4, -0.8),
    p2(-0.1, -1.08),
    p2(-1.28, -0.44),
    p2(-0.72, 0.0),
    p2(-1.28, 0.44),
    p2(-0.1, 1.08),
    p2(0.4, 0.8),
];

pub fn create_berserker_body() -> Part {
    let mut parts = vec![solid(extrude_outline(&BERSERKER_OUTLINE, 0.42, 0.12), 0.0, None)];
    parts.extend(glow_trim(&scale_outline(&BERSERKER_OUTLINE, 1.08), 0.33, 0.09, 0.09, 1.0));
    for side in [-1.0f32, 1.0] {
        let horn = place(0.75, 0.55, side * 0.72, 0.0, side * 0.5, -QUARTER_TURN, 1.0, 1.0, 1.0);
        parts.push(solid(cone(0.16, 0.9, 6), 0.25, Some(&horn)));
        let brow = place(0.98, 0.67, side * 0.2, 0.0, side * -0.45, 0.0, 1.0, 1.0, 1.0);
        parts.push(solid(box3(0.28, 0.08, 0.1), 1.0, Some(&brow)));
    }
    merge(parts)
}

pub fn create_berserker_spikes() -> Part {
    merge(
        [-0.42, 0.0, 0.42]
            .into_iter()
            .map(|z| solid(cone(0.13, 0.62, 5), 1.0, Some(&place(-0.6, 0.72, z, 0.0, 0.0, 0.9, 1.0, 1.0, 1.0))))
            .collect(),
    )
}

const BULWARK_SHELL_OUTLINE: [P2; 8] = [
    p2(1.35, 0.0),
    p2(0.82, -0.8),
    p2(-0.2, -0.98),
    p2(-1.08, -0.8),
    p2(-1.32, 0.0),
    p2(-1.08, 0.8),
    p2(-0.2, 0.98),
    p2(0.82, 0.8),
];

const BULWARK_CORE_OUTLINE: [P2; 6] =
    [p2(0.98, 0.0), p2(0.42, -0.46), p2(-0.3, -0.46), p2(-0.72, 0.0), p2(-0.3, 0.46), p2(0.42, 0.46)];

const BULWARK_FRONT_PLATE_OUTLINE: [P2; 5] =
    [p2(1.08, 0.0), p2(0.76, -0.28), p2(0.16, -0.28), p2(0.16, 0.28), p2(0.76, 0.28)];

const BULWARK_UPPER_TIER_Y: f32 = 0.62;

pub fn create_bulwark_shell() -> Part {
    let upper_tier = transform(
        extrude_outline(&scale_outline(&BULWARK_SHELL_OUTLINE, 0.8), 0.16, 0.06),
        &translate(0.0, BULWARK_UPPER_TIER_Y - 0.02, 0.0),
    );
    let front_plate = transform(
        extrude_outline(&BULWARK_FRONT_PLATE_OUTLINE, 0.08, 0.03),
        &translate(0.0, BULWARK_UPPER_TIER_Y + 0.2, 0.0),
    );
    let mut parts = vec![
        solid(extrude_outline(&BULWARK_SHELL_OUTLINE, 0.34, 0.14), 0.0, None),
        solid(upper_tier, 0.0, None),
        solid(front_plate, 0.5, None),
    ];
    parts.extend(glow_trim(&scale_outline(&BULWARK_SHELL_OUTLINE, 1.09), 0.3, 0.07, 0.1, 0.9));
    merge(parts)
}

/// Pulsing armor core and the glowing armor seams, tinted separately from the shell.
pub fn create_bulwark_core() -> Part {
    let core =
        transform(extrude_outline(&BULWARK_CORE_OUTLINE, 0.05, 0.02), &translate(0.0, BULWARK_UPPER_TIER_Y + 0.2, 0.0));
    let seam_y = BULWARK_UPPER_TIER_Y + 0.22;
    merge(vec![
        solid(core, 1.0, None),
        solid(bar(0.13, seam_y, -0.52, 0.6f32.atan2(0.7), 0.9, 0.05, 0.06), 1.0, None),
        solid(bar(0.13, seam_y, 0.52, (-0.6f32).atan2(0.7), 0.9, 0.05, 0.06), 1.0, None),
    ])
}

/// Irregular crystalline chunk used for every death shard.
pub fn create_shard() -> Part {
    let mut shard = octahedron(1.0);
    let jitter = [0.82, 1.18, 0.9, 1.05, 0.7, 1.12];
    for position in shard.positions.as_chunks_mut::<3>().0 {
        let [x, y, z] = *position;
        let axis = if x.abs() > 0.5 {
            if x > 0.0 { 0 } else { 1 }
        } else if y.abs() > 0.5 {
            if y > 0.0 { 2 } else { 3 }
        } else if z > 0.0 {
            4
        } else {
            5
        };
        let scale = jitter[axis];
        position[0] = x * scale * 1.1;
        position[1] = y * scale * 0.55;
        position[2] = z * scale;
    }
    let mut faceted = solid(recompute_flat_normals(shard), 0.3, None);
    for (vertex, glow) in faceted.glow.iter_mut().enumerate() {
        *glow = if (vertex / 3) % 2 == 0 { 0.55 } else { 0.12 };
    }
    faceted
}

// ---------------------------------------------------------------- towers

pub const TOWER_BASE_TOP: f32 = 5.0;

/// The dark plinth every tower stands on (tinted by the tower's accent).
pub fn create_tower_base() -> Part {
    merge(vec![
        solid(cylinder(11.2, 12.6, 4.2, 28), 0.0, Some(&translate(0.0, 2.1, 0.0))),
        solid(cylinder(10.2, 11.2, 0.8, 28), 0.0, Some(&translate(0.0, 4.6, 0.0))),
    ])
}

/// The glowing rim on the plinth's top edge: the original's white base stroke.
pub fn create_tower_rim() -> Part {
    solid(torus(11.35, 0.68, 6, 48), 1.0, Some(&place(0.0, 4.25, 0.0, QUARTER_TURN, 0.0, 0.0, 1.0, 1.0, 1.0)))
}

/// Unit-radius ring on the ground; instances scale it to each level's upgrade halo.
pub fn create_upgrade_ring() -> Part {
    solid(torus(1.0, 0.035, 4, 56), 1.0, Some(&place(0.0, 0.0, 0.0, QUARTER_TURN, 0.0, 0.0, 1.0, 1.0, 1.0)))
}

pub fn create_level_pip() -> Part {
    solid(box3(1.6, 1.1, 1.3), 1.0, None)
}

pub fn create_gun_head() -> Part {
    let top = TOWER_BASE_TOP;
    merge(vec![
        solid(
            cylinder(6.1, 7.0, 4.2, 6),
            0.0,
            Some(&place(0.0, top + 2.1, 0.0, 0.0, HALF_TURN / 6.0, 0.0, 1.0, 1.0, 1.0)),
        ),
        solid(
            torus(6.35, 0.32, 4, 6),
            1.0,
            Some(&place(0.0, top + 3.3, 0.0, QUARTER_TURN, 0.0, HALF_TURN / 6.0, 1.0, 1.0, 1.0)),
        ),
        solid(box3(3.2, 2.6, 5.4), 0.05, Some(&translate(-5.2, top + 2.7, 0.0))),
        solid(box3(0.5, 0.5, 4.2), 1.0, Some(&translate(-6.9, top + 3.2, 0.0))),
    ])
}

pub const GUN_BARREL_Y: f32 = TOWER_BASE_TOP + 3.1;

/// Unit-length barrel along +X; instances scale X by length and Y/Z by radius.
pub fn create_gun_barrel() -> Part {
    merge(vec![
        // Glows like the original's thick white barrel stroke.
        cylinder_along_x(1.0, 1.0, 10, 0.6, 0.0, 0.0),
        solid(box3(1.0, 0.35, 0.3), 1.0, Some(&translate(0.5, 1.02, 0.0))),
    ])
}

pub fn create_gun_muzzle() -> Part {
    cylinder_along_x(1.35, 1.8, 10, 1.0, -0.9, 0.0)
}

/// Unit-length glowing rail along +X.
pub fn create_rail() -> Part {
    solid(box3(1.0, 0.7, 0.7), 1.0, Some(&translate(0.5, 0.0, 0.0)))
}

pub const LASER_CRYSTAL_Y: f32 = TOWER_BASE_TOP + 4.4;

/// Unit diamond; instances scale it to each level's crystal length and girth.
pub fn create_laser_crystal() -> Part {
    solid(octahedron(1.0), 0.62, None)
}

pub fn create_laser_cradle() -> Part {
    let top = TOWER_BASE_TOP;
    let mut parts = vec![solid(cylinder(3.4, 4.0, 2.2, 16), 0.0, Some(&translate(0.0, top + 1.1, 0.0)))];
    for side in [-1.0, 1.0] {
        parts.push(solid(box3(7.0, 3.4, 1.3), 0.0, Some(&translate(-1.0, top + 3.2, side * 4.4))));
        parts.push(solid(box3(6.4, 0.4, 0.4), 1.0, Some(&translate(-1.0, top + 5.0, side * 4.4))));
    }
    merge(parts)
}

pub const MISSILE_RACK_Y: f32 = TOWER_BASE_TOP + 2.6;

pub fn create_missile_launcher() -> Part {
    let top = TOWER_BASE_TOP;
    let mut parts = vec![
        solid(box3(19.0, 1.4, 9.4), 0.0, Some(&translate(0.5, top + 0.9, 0.0))),
        solid(box3(2.6, 4.4, 9.4), 0.04, Some(&translate(-10.3, top + 2.4, 0.0))),
        solid(box3(0.4, 3.6, 7.4), 1.0, Some(&translate(-8.9, top + 2.6, 0.0))),
    ];
    for side in [-1.0, 1.0] {
        parts.push(solid(box3(19.0, 3.4, 1.2), 0.0, Some(&translate(0.5, top + 2.8, side * 4.1))));
        parts.push(solid(box3(18.0, 0.36, 0.5), 1.0, Some(&translate(0.8, top + 4.6, side * 4.1))));
    }
    merge(parts)
}

/// Missile centered near its midpoint, nose along +X.
pub fn create_missile() -> Part {
    let mut parts = vec![
        cylinder_along_x(1.65, 13.4, 10, 0.1, -7.4, 0.0),
        solid(cone(1.9, 5.2, 10), 1.0, Some(&place(8.6, 0.0, 0.0, 0.0, 0.0, -QUARTER_TURN, 1.0, 1.0, 1.0))),
        cylinder_along_x(1.35, 2.2, 10, 0.55, -9.6, 0.0),
    ];
    for fin in 0..4 {
        let angle = fin as f32 * QUARTER_TURN + QUARTER_TURN / 2.0;
        let placement = place(-6.4, angle.sin() * 1.9, angle.cos() * 1.9, angle, 0.0, 0.0, 1.0, 1.0, 1.0);
        parts.push(solid(box3(3.2, 0.3, 2.6), 0.0, Some(&placement)));
    }
    merge(parts)
}

pub const SLOW_CORE_Y: f32 = TOWER_BASE_TOP + 7.0;

pub fn create_slow_core() -> Part {
    merge(vec![
        solid(cylinder(1.6, 3.0, 6.0, 12), 0.0, Some(&translate(0.0, TOWER_BASE_TOP + 3.0 - SLOW_CORE_Y, 0.0))),
        solid(sphere(4.0, 20, 14, 0.0, FULL_TURN), 1.0, None),
        solid(torus(6.2, 0.45, 6, 36), 0.75, Some(&place(0.0, 0.0, 0.0, QUARTER_TURN + 0.5, 0.0, 0.0, 1.0, 1.0, 1.0))),
        solid(torus(5.4, 0.3, 6, 36), 0.55, Some(&place(0.0, 0.0, 0.0, QUARTER_TURN - 0.7, 0.0, 0.4, 1.0, 1.0, 1.0))),
    ])
}

pub fn create_orb_node() -> Part {
    solid(sphere(1.0, 10, 8, 0.0, FULL_TURN), 1.0, None)
}

pub const DRONE_PAD_TOP: f32 = TOWER_BASE_TOP + 1.5;

pub fn create_drone_pad() -> Part {
    merge(vec![
        solid(
            cylinder(9.4, 10.2, 1.5, 6),
            0.0,
            Some(&place(0.0, TOWER_BASE_TOP + 0.75, 0.0, 0.0, HALF_TURN / 6.0, 0.0, 1.0, 1.0, 1.0)),
        ),
        solid(
            torus(8.4, 0.36, 4, 6),
            1.0,
            Some(&place(0.0, DRONE_PAD_TOP, 0.0, QUARTER_TURN, 0.0, HALF_TURN / 6.0, 1.0, 1.0, 1.0)),
        ),
        solid(box3(0.7, 0.2, 7.0), 0.8, Some(&translate(-2.6, DRONE_PAD_TOP, 0.0))),
        solid(box3(0.7, 0.2, 7.0), 0.8, Some(&translate(2.6, DRONE_PAD_TOP, 0.0))),
        solid(box3(5.2, 0.2, 0.7), 0.8, Some(&translate(0.0, DRONE_PAD_TOP, 0.0))),
    ])
}

/// The coil (not the plinth) is scaled up from its base on the plinth top.
pub const TESLA_COIL_SCALE: f32 = 1.2;
pub const TESLA_TOP_Y: f32 = TOWER_BASE_TOP + 17.5 * TESLA_COIL_SCALE;

pub fn create_tesla_coil() -> Part {
    let s = TESLA_COIL_SCALE;
    let ring = |radius: f32, tube: f32, height: f32| {
        let placement = place(0.0, TOWER_BASE_TOP + height * s, 0.0, QUARTER_TURN, 0.0, 0.0, 1.0, 1.0, 1.0);
        solid(torus(radius * s, tube * s, 6, 24), 1.0, Some(&placement))
    };
    merge(vec![
        solid(cylinder(1.5 * s, 2.6 * s, 15.0 * s, 12), 0.0, Some(&translate(0.0, TOWER_BASE_TOP + 7.5 * s, 0.0))),
        ring(4.6, 0.55, 3.5),
        ring(3.9, 0.5, 7.3),
        ring(3.2, 0.45, 11.0),
        solid(sphere(3.3 * s, 16, 12, 0.0, FULL_TURN), 1.0, Some(&translate(0.0, TESLA_TOP_Y, 0.0))),
    ])
}

// ---------------------------------------------------------------- flyers & board

pub fn create_drone_body() -> Part {
    let mut parts = vec![
        solid(box3(7.8, 2.2, 7.8), 0.0, None),
        solid(box3(3.8, 0.5, 1.8), 1.0, Some(&translate(0.0, 1.2, 0.0))),
        solid(bar(0.0, 0.0, 0.0, PI / 4.0, 19.5, 1.0, 1.2), 0.0, None),
        solid(bar(0.0, 0.0, 0.0, -PI / 4.0, 19.5, 1.0, 1.2), 0.0, None),
    ];
    for x in [-6.9, 6.9] {
        for z in [-6.9, 6.9] {
            parts.push(solid(cylinder(1.5, 1.5, 1.8, 10), 0.8, Some(&translate(x, 0.2, z))));
        }
    }
    merge(parts)
}

pub fn create_portal() -> Part {
    let flat = |y: f32| place(0.0, y, 0.0, QUARTER_TURN, 0.0, 0.0, 1.0, 1.0, 1.0);
    let mut parts = vec![
        solid(cylinder(0.96, 1.0, 0.035, 40), 0.05, Some(&translate(0.0, 0.02, 0.0))),
        solid(torus(1.0, 0.06, 6, 48), 1.0, Some(&flat(0.05))),
        solid(torus(0.7, 0.028, 4, 40), 0.8, Some(&flat(0.05))),
    ];
    for pylon in 0..6 {
        let angle = pylon as f32 / 6.0 * FULL_TURN;
        let placement = place(angle.cos() * 1.1, 0.12, angle.sin() * 1.1, 0.0, -angle, 0.0, 1.0, 1.0, 1.0);
        parts.push(solid(box3(0.09, 0.24, 0.16), 0.5, Some(&placement)));
    }
    merge(parts)
}

pub fn create_spawn_gate(road_width: f32) -> Part {
    let half_span = road_width / 2.0 + 3.2;
    merge(vec![
        solid(box3(2.6, 15.0, 2.6), 0.05, Some(&translate(0.0, 7.5, -half_span))),
        solid(box3(2.6, 15.0, 2.6), 0.05, Some(&translate(0.0, 7.5, half_span))),
        solid(box3(2.4, 1.8, half_span * 2.0 + 2.6), 0.1, Some(&translate(0.0, 15.6, 0.0))),
        solid(box3(0.6, 0.6, half_span * 2.0), 1.0, Some(&translate(0.0, 14.4, 0.0))),
        solid(box3(0.6, 14.0, 0.6), 1.0, Some(&translate(0.0, 7.0, -half_span + 1.4))),
        solid(box3(0.6, 14.0, 0.6), 1.0, Some(&translate(0.0, 7.0, half_span - 1.4))),
    ])
}

// ---------------------------------------------------------------- effect geometry

/// A flat quad on the ground plane (XZ) spanning [min_x, max_x] x [-half_depth, half_depth];
/// uv.x runs along +X and uv.y from +Z (0) to -Z (1). Interleaved `position(3) uv(2)`.
fn flat_quad(min_x: f32, max_x: f32, half_depth: f32) -> Mesh {
    let corners = [
        [min_x, half_depth, 0.0, 0.0],
        [max_x, half_depth, 1.0, 0.0],
        [max_x, -half_depth, 1.0, 1.0],
        [min_x, -half_depth, 0.0, 1.0],
    ];
    let mut vertices = Vec::with_capacity(30);
    for index in [0, 1, 2, 0, 2, 3] {
        let [x, z, u, v] = corners[index];
        vertices.extend([x, 0.0, z, u, v]);
    }
    Mesh { vertices, vertex_count: 6 }
}

/// Flat unit quad on the ground plane, centered.
pub fn create_flat_quad() -> Mesh {
    flat_quad(-0.5, 0.5, 0.5)
}

/// Flat unit ribbon spanning x = 0..1 with uv.y across its width.
pub fn create_ribbon_quad() -> Mesh {
    flat_quad(0.0, 1.0, 0.5)
}

/// Flat disc quad of radius 1 whose uv spans the full square (shaders draw the circle).
pub fn create_range_quad() -> Mesh {
    flat_quad(-1.0, 1.0, 1.0)
}
