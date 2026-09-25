use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use ta_net::{CHECKSUM_INTERVAL, ClientMsg, Lockstep, ServerMsg, TICKS_PER_TURN, Turn};
use ta_sim::{Command, FxVec2, PlayerId, Sim, TICK_RATE, UnitId};

use crate::AppState;
use crate::net::{Connection, NetEvent, close_connection};

const TICK_SECONDS: f32 = 1.0 / TICK_RATE as f32;
/// Online: holding more ticks than this means we lag behind the server, so the
/// simulation runs faster until it catches up.
const MAX_BUFFERED_TICKS: u32 = 2 * TICKS_PER_TURN;
/// Bounds the work done in one frame, so a slow frame cannot snowball.
const MAX_TICKS_PER_FRAME: u32 = 10;

pub fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::InGame), spawn_hud)
        .add_systems(OnExit(AppState::InGame), end_session)
        .add_systems(
            Update,
            (drive_simulation, update_hud, leave_on_escape)
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
}

/// A running game: the lockstep simulation and the local player's view of it.
#[derive(Resource)]
pub struct Session {
    pub lockstep: Lockstep,
    pub local_player: PlayerId,
    /// Solo: commands waiting for the next locally generated turn.
    pending: Vec<Command>,
    /// Real time not simulated yet, in seconds.
    accumulator: f32,
    /// Unit positions before the last tick, to interpolate rendering between ticks.
    previous_positions: HashMap<UnitId, FxVec2>,
    /// Latest noteworthy network event, shown in the HUD.
    notice: Option<String>,
}

impl Session {
    pub fn new(sim: Sim, local_player: PlayerId) -> Session {
        Session {
            lockstep: Lockstep::new(sim),
            local_player,
            pending: Vec::new(),
            accumulator: 0.0,
            previous_positions: HashMap::default(),
            notice: None,
        }
    }

    /// Online, sends the command to the server; solo, keeps it for the next turn.
    pub fn issue(&mut self, command: Command, connection: Option<&mut Connection>) {
        match connection {
            Some(connection) => connection.send(&ClientMsg::Command(command)),
            None => self.pending.push(command),
        }
    }

    /// Progress from the previous tick to the current one, in `[0, 1]`.
    pub fn alpha(&self) -> f32 {
        (self.accumulator / TICK_SECONDS).min(1.0)
    }

    pub fn previous_position(&self, id: UnitId) -> Option<FxVec2> {
        self.previous_positions.get(&id).copied()
    }
}

/// Feeds turns to the lockstep and runs as many ticks as real time allows.
pub fn drive_simulation(
    time: Res<Time>,
    mut session: ResMut<Session>,
    mut connection: Option<NonSendMut<Connection>>,
) {
    let session = &mut *session;
    if let Some(connection) = connection.as_mut() {
        while let Some(event) = connection.poll() {
            match event {
                NetEvent::Message(ServerMsg::Turn(turn)) => session.lockstep.push_turn(turn),
                NetEvent::Message(ServerMsg::Desync { turn }) => {
                    session.notice = Some(format!("Desync detected at turn {turn}"));
                }
                NetEvent::Message(ServerMsg::PlayerLeft { player }) => {
                    session.notice = Some(format!("Player {} left", player + 1));
                }
                NetEvent::Closed(reason) => {
                    session.notice = Some(format!("Disconnected: {reason}"))
                }
                NetEvent::Opened | NetEvent::Message(_) => {}
            }
        }
    }

    session.accumulator =
        (session.accumulator + time.delta_secs()).min(MAX_TICKS_PER_FRAME as f32 * TICK_SECONDS);
    for _ in 0..MAX_TICKS_PER_FRAME {
        let lagging = session.lockstep.buffered_ticks() > MAX_BUFFERED_TICKS;
        if session.accumulator < TICK_SECONDS && !lagging {
            break;
        }
        if session.lockstep.buffered_ticks() == 0 {
            if connection.is_some() {
                // Wait for the server's next turn.
                session.accumulator = session.accumulator.min(TICK_SECONDS);
                break;
            }
            let player = session.local_player;
            let commands = session
                .pending
                .drain(..)
                .map(|command| (player, command))
                .collect();
            session.lockstep.push_turn(Turn {
                number: session.lockstep.next_turn(),
                commands,
            });
        }
        session.previous_positions = session
            .lockstep
            .sim()
            .units()
            .iter()
            .map(|unit| (unit.id, unit.pos))
            .collect();
        let completed = session.lockstep.step();
        session.accumulator = (session.accumulator - TICK_SECONDS).max(0.0);
        if let (Some(turn), Some(connection)) = (completed, connection.as_mut())
            && turn % CHECKSUM_INTERVAL == 0
        {
            connection.send(&ClientMsg::Checksum {
                turn,
                hash: session.lockstep.sim().checksum(),
            });
        }
    }
}

#[derive(Component)]
struct Hud;

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        Hud,
        Text::default(),
        TextFont::from_font_size(16.0),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            ..default()
        },
        DespawnOnExit(AppState::InGame),
    ));
    commands.spawn((
        Text::new(
            "Left click/drag: select   Right click: move   S: stop   Arrows: scroll   Wheel: zoom   Esc: menu",
        ),
        TextFont::from_font_size(14.0),
        TextColor(Color::srgb(0.75, 0.75, 0.75)),
        Node { position_type: PositionType::Absolute, bottom: Val::Px(8.0), left: Val::Px(8.0), ..default() },
        DespawnOnExit(AppState::InGame),
    ));
}

fn update_hud(
    session: Res<Session>,
    connection: Option<NonSend<Connection>>,
    mut hud: Single<&mut Text, With<Hud>>,
) {
    let mut text = match connection {
        Some(_) => format!(
            "Online - player {}   buffer: {} ticks",
            session.local_player + 1,
            session.lockstep.buffered_ticks()
        ),
        None => "Solo".to_owned(),
    };
    let seconds = session.lockstep.sim().tick() / TICK_RATE as u64;
    text += &format!("   {:02}:{:02}", seconds / 60, seconds % 60);
    if let Some(notice) = &session.notice {
        text += "\n";
        text += notice;
    }
    hud.0 = text;
}

fn leave_on_escape(keys: Res<ButtonInput<KeyCode>>, mut next: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Menu);
    }
}

fn end_session(mut commands: Commands) {
    commands.remove_resource::<Session>();
    // The server tells the other players that we left.
    close_connection(&mut commands);
}
