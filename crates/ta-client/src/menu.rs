use bevy::prelude::*;
use ta_net::{ClientMsg, DEFAULT_PORT, PROTOCOL_VERSION, ServerMsg};
use ta_sim::Sim;

use crate::AppState;
use crate::net::{Connection, NetEvent, close_connection};
use crate::session::Session;

/// Solo games always use the same map for now.
const SOLO_SEED: u64 = 1;
const BUTTON_COLOR: Color = Color::srgb(0.16, 0.18, 0.22);
const BUTTON_HOVER_COLOR: Color = Color::srgb(0.24, 0.27, 0.33);

pub fn plugin(app: &mut App) {
    let (autostart, server_url) = launch_options();
    app.insert_resource(ServerUrl(server_url))
        .init_resource::<MenuStatus>()
        .add_systems(OnEnter(AppState::Menu), spawn_menu)
        .add_systems(
            Update,
            (press_buttons, poll_connection, show_status)
                .chain()
                .run_if(in_state(AppState::Menu)),
        );
    if let Some(mode) = autostart {
        app.add_systems(
            Startup,
            move |mut commands: Commands,
                  url: Res<ServerUrl>,
                  mut status: ResMut<MenuStatus>,
                  mut next: ResMut<NextState<AppState>>| {
                start(mode, &mut commands, &url.0, &mut status, &mut next);
            },
        );
    }
}

/// A way to play; also the component of the menu button that starts it.
#[derive(Component, Clone, Copy)]
enum Mode {
    Solo,
    Online,
}

#[derive(Resource)]
struct ServerUrl(String);

/// Connection progress or error, shown under the buttons.
#[derive(Resource, Default)]
struct MenuStatus(String);

#[derive(Component)]
struct StatusText;

fn spawn_menu(mut commands: Commands, status: Res<MenuStatus>) {
    commands.spawn((Camera2d, DespawnOnExit(AppState::Menu)));
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(16.0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.05, 0.06, 0.08)),
        DespawnOnExit(AppState::Menu),
        children![
            (Text::new("TA Reborn"), TextFont::from_font_size(56.0)),
            menu_button("Solo", Mode::Solo),
            menu_button("Online", Mode::Online),
            (
                StatusText,
                Text::new(status.0.clone()),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgb(0.7, 0.7, 0.7)),
            ),
        ],
    ));
}

fn menu_button(label: &'static str, mode: Mode) -> impl Bundle {
    (
        mode,
        Button,
        Node {
            width: Val::Px(240.0),
            padding: UiRect::all(Val::Px(12.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(BUTTON_COLOR),
        children![(Text::new(label), TextFont::from_font_size(24.0))],
    )
}

fn press_buttons(
    mut commands: Commands,
    mut buttons: Query<(&Interaction, &Mode, &mut BackgroundColor), Changed<Interaction>>,
    url: Res<ServerUrl>,
    mut status: ResMut<MenuStatus>,
    mut next: ResMut<NextState<AppState>>,
) {
    for (interaction, &mode, mut color) in &mut buttons {
        match interaction {
            Interaction::Pressed => start(mode, &mut commands, &url.0, &mut status, &mut next),
            Interaction::Hovered => color.0 = BUTTON_HOVER_COLOR,
            Interaction::None => color.0 = BUTTON_COLOR,
        }
    }
}

fn start(
    mode: Mode,
    commands: &mut Commands,
    url: &str,
    status: &mut MenuStatus,
    next: &mut NextState<AppState>,
) {
    // Drop any connection attempt still in progress.
    close_connection(commands);
    match mode {
        Mode::Solo => {
            commands.insert_resource(Session::new(Sim::new(SOLO_SEED, 1), 0));
            status.0.clear();
            next.set(AppState::InGame);
        }
        Mode::Online => {
            status.0 = format!("Connecting to {url}...");
            let url = url.to_owned();
            commands.queue(move |world: &mut World| match Connection::open(&url) {
                Ok(connection) => world.insert_non_send(connection),
                Err(err) => {
                    world.resource_mut::<MenuStatus>().0 = format!("Connection failed: {err}")
                }
            });
        }
    }
}

fn poll_connection(
    mut commands: Commands,
    connection: Option<NonSendMut<Connection>>,
    mut status: ResMut<MenuStatus>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(mut connection) = connection else {
        return;
    };
    while let Some(event) = connection.poll() {
        match event {
            NetEvent::Opened => connection.send(&ClientMsg::Join {
                version: PROTOCOL_VERSION,
            }),
            NetEvent::Message(ServerMsg::Waiting) => status.0 = "Waiting for an opponent...".into(),
            NetEvent::Message(ServerMsg::Start { seed, players, you }) => {
                commands.insert_resource(Session::new(Sim::new(seed, players), you));
                status.0.clear();
                next.set(AppState::InGame);
                // The turns that follow are read by the game.
                return;
            }
            NetEvent::Message(ServerMsg::VersionMismatch { server }) => {
                status.0 =
                    format!("Server runs protocol v{server}, this client v{PROTOCOL_VERSION}");
                close_connection(&mut commands);
                return;
            }
            NetEvent::Closed(reason) => {
                status.0 = format!("Connection failed: {reason}");
                close_connection(&mut commands);
                return;
            }
            NetEvent::Message(_) => {}
        }
    }
}

fn show_status(status: Res<MenuStatus>, mut text: Single<&mut Text, With<StatusText>>) {
    if status.is_changed() {
        text.0.clone_from(&status.0);
    }
}

/// Mode to start right away (skipping the menu) and relay server address, from the
/// command line.
#[cfg(not(target_arch = "wasm32"))]
fn launch_options() -> (Option<Mode>, String) {
    let mut mode = None;
    let mut url = format!("ws://127.0.0.1:{DEFAULT_PORT}");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "solo" => mode = Some(Mode::Solo),
            "online" => mode = Some(Mode::Online),
            "--server" => url = args.next().expect("--server needs an address"),
            _ => panic!(
                "unknown argument {arg:?}; usage: ta-client [solo|online] [--server ws://HOST:PORT]"
            ),
        }
    }
    (mode, url)
}

/// On the web: no autostart, and the relay address comes from `?server=` in the page
/// URL, defaulting to the host that served the page.
#[cfg(target_arch = "wasm32")]
fn launch_options() -> (Option<Mode>, String) {
    let location = web_sys::window().expect("no browser window").location();
    let query = location.search().unwrap_or_default();
    let server = query
        .trim_start_matches('?')
        .split('&')
        .find_map(|pair| pair.strip_prefix("server="));
    let url = match server {
        Some(url) => url.to_owned(),
        None => format!(
            "ws://{}:{DEFAULT_PORT}",
            location.hostname().unwrap_or_default()
        ),
    };
    (None, url)
}
