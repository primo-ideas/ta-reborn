use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use ta_sim::{Fx, FxVec2, PlayerId, Terrain, UnitId};

use crate::AppState;
use crate::session::{Session, drive_simulation};

const PLAYER_COLORS: [Color; 2] = [Color::srgb(0.15, 0.35, 0.9), Color::srgb(0.85, 0.15, 0.1)];

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::InGame), spawn_scene)
        .add_systems(
            Update,
            sync_unit_views
                .after(drive_simulation)
                .run_if(in_state(AppState::InGame)),
        );
}

/// The 3D model of a simulated unit.
#[derive(Component)]
pub struct UnitView {
    pub id: UnitId,
    pub owner: PlayerId,
    yaw: f32,
}

pub fn to_vec2(p: FxVec2) -> Vec2 {
    Vec2::new(p.x.to_f32(), p.z.to_f32())
}

/// Height of the rendered ground. Until water has gameplay, units drive on its surface.
pub fn ground_height(terrain: &Terrain, x: f32, z: f32) -> f32 {
    let p = FxVec2::new(Fx::from_f32(x), Fx::from_f32(z));
    terrain.height_at(p).to_f32().max(0.0)
}

fn spawn_scene(
    mut commands: Commands,
    session: Res<Session>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let sim = session.lockstep.sim();
    let size = sim.terrain().world_size().to_f32();

    commands.spawn((
        Mesh3d(meshes.add(terrain_mesh(sim.terrain()))),
        MeshMaterial3d(materials.add(StandardMaterial {
            perceptual_roughness: 0.95,
            ..default()
        })),
        DespawnOnExit(AppState::InGame),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(size, size))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.1, 0.3, 0.45, 0.75),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.2,
            ..default()
        })),
        Transform::from_xyz(size / 2.0, 0.0, size / 2.0),
        DespawnOnExit(AppState::InGame),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        // Low sun from the east, so that slopes show.
        Transform::default().looking_to(Vec3::new(-1.0, -0.8, -0.5), Vec3::Y),
        DespawnOnExit(AppState::InGame),
    ));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 200.0,
        ..default()
    });

    // Placeholder tank: hull in team color, turret and barrel pointing forward (-Z).
    let hull = meshes.add(Cuboid::new(2.2, 0.9, 3.0));
    let turret = meshes.add(Cuboid::new(1.3, 0.6, 1.5));
    let barrel = meshes.add(Cuboid::new(0.25, 0.25, 1.6));
    let steel = materials.add(Color::srgb(0.25, 0.25, 0.28));
    let team: Vec<_> = PLAYER_COLORS
        .iter()
        .map(|&color| materials.add(color))
        .collect();
    let center = Vec2::splat(size / 2.0);
    for unit in sim.units() {
        commands.spawn((
            UnitView {
                id: unit.id,
                owner: unit.owner,
                yaw: yaw_towards(center - to_vec2(unit.pos)),
            },
            Transform::default(),
            Visibility::default(),
            DespawnOnExit(AppState::InGame),
            children![
                (
                    Mesh3d(hull.clone()),
                    MeshMaterial3d(team[unit.owner as usize].clone()),
                    Transform::from_xyz(0.0, 0.45, 0.0),
                ),
                (
                    Mesh3d(turret.clone()),
                    MeshMaterial3d(steel.clone()),
                    Transform::from_xyz(0.0, 1.2, 0.2)
                ),
                (
                    Mesh3d(barrel.clone()),
                    MeshMaterial3d(steel.clone()),
                    Transform::from_xyz(0.0, 1.2, -1.2)
                ),
            ],
        ));
    }
}

fn terrain_mesh(terrain: &Terrain) -> Mesh {
    let n = terrain.size() + 1;
    let cell = Terrain::CELL_SIZE.to_f32();
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for z in 0..n {
        for x in 0..n {
            let height = terrain.vertex_height(x, z).to_f32();
            positions.push([x as f32 * cell, height, z as f32 * cell]);
            colors.push(ground_color(height).to_f32_array());
        }
    }
    let mut indices = Vec::new();
    for z in 0..terrain.size() {
        for x in 0..terrain.size() {
            let i = z * n + x;
            // Two triangles per cell, counter-clockwise seen from above.
            indices.extend([i, i + n, i + 1, i + 1, i + n, i + n + 1]);
        }
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
    .with_computed_smooth_normals()
}

/// Sand on the shores, grass, then rock on the hilltops.
fn ground_color(height: f32) -> LinearRgba {
    let stops = [
        (-3.0, Color::srgb(0.45, 0.4, 0.3)),
        (0.8, Color::srgb(0.76, 0.7, 0.5)),
        (2.5, Color::srgb(0.35, 0.5, 0.2)),
        (10.0, Color::srgb(0.22, 0.38, 0.15)),
        (16.0, Color::srgb(0.45, 0.4, 0.33)),
        (21.0, Color::srgb(0.6, 0.58, 0.55)),
    ];
    let above = stops
        .iter()
        .position(|&(h, _)| h > height)
        .unwrap_or(stops.len());
    if above == 0 || above == stops.len() {
        return stops[above.min(stops.len() - 1)].1.into();
    }
    let ((h0, c0), (h1, c1)) = (stops[above - 1], stops[above]);
    LinearRgba::from(c0).mix(&c1.into(), (height - h0) / (h1 - h0))
}

fn sync_unit_views(session: Res<Session>, mut views: Query<(&mut UnitView, &mut Transform)>) {
    let sim = session.lockstep.sim();
    for (mut view, mut transform) in &mut views {
        let Some(unit) = sim.unit(view.id) else {
            continue;
        };
        let current = to_vec2(unit.pos);
        let previous = session.previous_position(view.id).map_or(current, to_vec2);
        let motion = current - previous;
        if motion.length_squared() > 1e-6 {
            view.yaw = yaw_towards(motion);
        }
        let p = previous.lerp(current, session.alpha());
        *transform = Transform::from_xyz(p.x, ground_height(sim.terrain(), p.x, p.y), p.y)
            .with_rotation(Quat::from_rotation_y(view.yaw));
    }
}

/// Rotation around Y that points a model's forward (-Z) along a ground direction.
fn yaw_towards(direction: Vec2) -> f32 {
    f32::atan2(-direction.x, -direction.y)
}
