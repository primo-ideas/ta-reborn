use bevy::input::mouse::{AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;

use crate::AppState;
use crate::scene::{ground_height, to_vec2};
use crate::session::Session;

/// Tilt below the horizon, in radians (about 55°).
const PITCH: f32 = 0.96;
const MIN_DISTANCE: f32 = 25.0;
const MAX_DISTANCE: f32 = 250.0;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::InGame), spawn_camera)
        .add_systems(Update, control_camera.run_if(in_state(AppState::InGame)));
}

/// Strategy camera looking north at a point of the ground from `distance` away.
#[derive(Component)]
struct RtsCamera {
    focus: Vec2,
    distance: f32,
}

fn spawn_camera(mut commands: Commands, session: Res<Session>) {
    // Start above the local player's army.
    let army: Vec<Vec2> = session
        .lockstep
        .sim()
        .units()
        .iter()
        .filter(|unit| unit.owner == session.local_player)
        .map(|unit| to_vec2(unit.pos))
        .collect();
    let focus = army.iter().sum::<Vec2>() / army.len() as f32;
    commands.spawn((
        Camera3d::default(),
        RtsCamera {
            focus,
            distance: 90.0,
        },
        DespawnOnExit(AppState::InGame),
    ));
}

fn control_camera(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    session: Res<Session>,
    camera: Single<(&mut RtsCamera, &mut Transform)>,
) {
    let (mut camera, mut transform) = camera.into_inner();
    let terrain = session.lockstep.sim().terrain();

    let mut pan = Vec2::ZERO;
    for (key, direction) in [
        (KeyCode::ArrowLeft, Vec2::NEG_X),
        (KeyCode::ArrowRight, Vec2::X),
        (KeyCode::ArrowUp, Vec2::NEG_Y),
        (KeyCode::ArrowDown, Vec2::Y),
    ] {
        if keys.pressed(key) {
            pan += direction;
        }
    }
    // Pan faster when zoomed out.
    let focus = camera.focus + pan * camera.distance * time.delta_secs();
    camera.focus = focus.clamp(Vec2::ZERO, Vec2::splat(terrain.world_size().to_f32()));

    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 100.0,
    };
    camera.distance = (camera.distance * 0.9f32.powf(notches)).clamp(MIN_DISTANCE, MAX_DISTANCE);

    let focus = Vec3::new(
        camera.focus.x,
        ground_height(terrain, camera.focus.x, camera.focus.y),
        camera.focus.y,
    );
    let offset = Vec3::new(0.0, PITCH.sin(), PITCH.cos()) * camera.distance;
    *transform = Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y);
}
