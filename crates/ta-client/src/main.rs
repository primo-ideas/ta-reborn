//! Bevy client: renders the simulation in 3D and turns player input into commands.
//!
//! Native usage: `ta-client [solo|online] [--server ws://HOST:PORT]`.
//! Web: `?server=ws://HOST:PORT` in the page URL.

mod camera;
mod menu;
mod net;
mod scene;
mod selection;
mod session;

use bevy::prelude::*;

#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
enum AppState {
    #[default]
    Menu,
    InGame,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "TA Reborn".into(),
                // Web only: render into the page's canvas and follow its size.
                canvas: Some("#bevy".into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .init_state::<AppState>()
        .add_plugins((
            menu::plugin,
            session::plugin,
            scene::plugin,
            camera::plugin,
            selection::plugin,
        ))
        .run();
}
