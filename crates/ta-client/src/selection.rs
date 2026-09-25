use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use ta_sim::{Command, Fx, FxVec2, Terrain};

use crate::AppState;
use crate::net::Connection;
use crate::scene::{UnitView, ground_height};
use crate::session::Session;

/// Cursor travel, in pixels, below which a press and release is a click.
const DRAG_THRESHOLD: f32 = 5.0;
/// How close, in pixels, a click must be to a unit to select it.
const CLICK_RADIUS: f32 = 25.0;
const SELECTION_COLOR: Color = Color::srgb(0.4, 1.0, 0.4);

pub fn plugin(app: &mut App) {
    app.init_resource::<DragStart>()
        .add_systems(OnEnter(AppState::InGame), spawn_selection_box)
        .add_systems(
            Update,
            (select_units, order_units, draw_selection_rings).run_if(in_state(AppState::InGame)),
        );
}

#[derive(Component)]
struct Selected;

/// Where the left button was pressed, while it is held.
#[derive(Resource, Default)]
struct DragStart(Option<Vec2>);

#[derive(Component)]
struct SelectionBox;

fn spawn_selection_box(mut commands: Commands) {
    commands.spawn((
        SelectionBox,
        Node {
            position_type: PositionType::Absolute,
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor::all(SELECTION_COLOR),
        BackgroundColor(SELECTION_COLOR.with_alpha(0.1)),
        Visibility::Hidden,
        DespawnOnExit(AppState::InGame),
    ));
}

fn select_units(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    session: Res<Session>,
    units: Query<(Entity, &UnitView, &GlobalTransform)>,
    selected: Query<Entity, With<Selected>>,
    mut drag: ResMut<DragStart>,
    selection_box: Single<(&mut Node, &mut Visibility), With<SelectionBox>>,
) {
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        drag.0 = Some(cursor);
    }
    let Some(start) = drag.0 else { return };
    let (min, max) = (start.min(cursor), start.max(cursor));
    let is_drag = min.distance(max) > DRAG_THRESHOLD;
    let (mut node, mut visibility) = selection_box.into_inner();
    node.left = Val::Px(min.x);
    node.top = Val::Px(min.y);
    node.width = Val::Px(max.x - min.x);
    node.height = Val::Px(max.y - min.y);
    *visibility = if is_drag {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    drag.0 = None;
    *visibility = Visibility::Hidden;
    for entity in &selected {
        commands.entity(entity).remove::<Selected>();
    }
    // Only our own units can be selected, by where they appear on screen.
    let (camera, camera_transform) = *camera;
    let on_screen = units
        .iter()
        .filter(|(_, view, _)| view.owner == session.local_player)
        .filter_map(|(entity, _, transform)| {
            let screen = camera
                .world_to_viewport(camera_transform, transform.translation())
                .ok()?;
            Some((entity, screen))
        });
    if is_drag {
        for (entity, screen) in on_screen {
            if screen.cmpge(min).all() && screen.cmple(max).all() {
                commands.entity(entity).insert(Selected);
            }
        }
    } else if let Some((entity, _)) = on_screen
        .map(|(entity, screen)| (entity, screen.distance(cursor)))
        .filter(|&(_, distance)| distance < CLICK_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
    {
        commands.entity(entity).insert(Selected);
    }
}

fn order_units(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut session: ResMut<Session>,
    mut connection: Option<NonSendMut<Connection>>,
    selected: Query<&UnitView, With<Selected>>,
) {
    if selected.is_empty() {
        return;
    }
    // Sorted ids give stable formation slots.
    let units = || {
        let mut ids: Vec<_> = selected.iter().map(|view| view.id).collect();
        ids.sort();
        ids
    };
    let command = if keys.just_pressed(KeyCode::KeyS) {
        Command::Stop { units: units() }
    } else if mouse.just_pressed(MouseButton::Right) {
        let (camera, camera_transform) = *camera;
        let Some(target) = window
            .cursor_position()
            .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor).ok())
            .and_then(|ray| ground_point(ray, session.lockstep.sim().terrain()))
        else {
            return;
        };
        let target = FxVec2::new(Fx::from_f32(target.x), Fx::from_f32(target.z));
        Command::Move {
            units: units(),
            target,
        }
    } else {
        return;
    };
    session.issue(command, connection.as_deref_mut());
}

/// First point where a ray meets the ground, found by marching along it.
fn ground_point(ray: Ray3d, terrain: &Terrain) -> Option<Vec3> {
    const STEP: f32 = 1.0;
    const MAX_DISTANCE: f32 = 1000.0;
    let below_ground = |t: f32| {
        let p = ray.get_point(t);
        p.y <= ground_height(terrain, p.x, p.z)
    };
    let mut t = 0.0;
    while t < MAX_DISTANCE {
        if below_ground(t + STEP) {
            // Refine by bisection within the last step.
            let (mut above, mut below) = (t, t + STEP);
            for _ in 0..12 {
                let mid = (above + below) / 2.0;
                if below_ground(mid) {
                    below = mid;
                } else {
                    above = mid;
                }
            }
            return Some(ray.get_point(below));
        }
        t += STEP;
    }
    None
}

fn draw_selection_rings(mut gizmos: Gizmos, selected: Query<&GlobalTransform, With<Selected>>) {
    for transform in &selected {
        // Circles are drawn in the XY plane: tilt them flat onto the ground.
        let flat = Isometry3d::new(
            transform.translation() + Vec3::Y * 0.1,
            Quat::from_rotation_x(FRAC_PI_2),
        );
        gizmos.circle(flat, 2.2, SELECTION_COLOR);
    }
}
