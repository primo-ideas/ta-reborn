use std::collections::VecDeque;

use ta_sim::Sim;

use crate::{TICKS_PER_TURN, Turn};

/// Runs a [`Sim`] from an ordered stream of turns. Solo and online games both drive
/// the simulation through this, so the same turns always produce the same game.
pub struct Lockstep {
    sim: Sim,
    turns: VecDeque<Turn>,
    next_turn: u32,
}

impl Lockstep {
    pub fn new(sim: Sim) -> Lockstep {
        Lockstep {
            sim,
            turns: VecDeque::new(),
            next_turn: 0,
        }
    }

    pub fn sim(&self) -> &Sim {
        &self.sim
    }

    /// Number that the next pushed turn must have.
    pub fn next_turn(&self) -> u32 {
        self.next_turn
    }

    /// Queues the next turn. Turns must be pushed in order, without gaps.
    pub fn push_turn(&mut self, turn: Turn) {
        assert_eq!(turn.number, self.next_turn, "turns must arrive in order");
        self.next_turn += 1;
        self.turns.push_back(turn);
    }

    /// Ticks that can run with the turns received so far.
    pub fn buffered_ticks(&self) -> u32 {
        self.turns.len() as u32 * TICKS_PER_TURN - self.tick_in_turn()
    }

    /// Runs one tick; requires `buffered_ticks() > 0`. Returns the number of the turn
    /// that this tick completed, if any.
    pub fn step(&mut self) -> Option<u32> {
        let tick_in_turn = self.tick_in_turn();
        let turn = self.turns.front().expect("step() needs a buffered turn");
        if tick_in_turn == 0 {
            for (player, command) in &turn.commands {
                self.sim.apply(*player, command);
            }
        }
        self.sim.step();
        if tick_in_turn + 1 < TICKS_PER_TURN {
            return None;
        }
        self.turns.pop_front().map(|turn| turn.number)
    }

    fn tick_in_turn(&self) -> u32 {
        (self.sim.tick() % TICKS_PER_TURN as u64) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_sim::{Command, Fx, FxVec2};

    fn turn(number: u32, commands: Vec<(u8, Command)>) -> Turn {
        Turn { number, commands }
    }

    #[test]
    fn runs_only_buffered_ticks_and_reports_completed_turns() {
        let mut lockstep = Lockstep::new(Sim::new(1, 1));
        assert_eq!(lockstep.buffered_ticks(), 0);
        lockstep.push_turn(turn(0, vec![]));
        lockstep.push_turn(turn(1, vec![]));
        assert_eq!(lockstep.buffered_ticks(), 2 * TICKS_PER_TURN);
        let completed: Vec<Option<u32>> =
            (0..2 * TICKS_PER_TURN).map(|_| lockstep.step()).collect();
        assert_eq!(completed, [None, None, Some(0), None, None, Some(1)]);
        assert_eq!(lockstep.buffered_ticks(), 0);
    }

    #[test]
    fn commands_apply_at_turn_start() {
        let mut lockstep = Lockstep::new(Sim::new(1, 1));
        let id = lockstep.sim().units()[0].id;
        let target = FxVec2::new(Fx::from_int(10), Fx::from_int(10));
        lockstep.push_turn(turn(
            0,
            vec![(
                0,
                Command::Move {
                    units: vec![id],
                    target,
                },
            )],
        ));
        lockstep.step();
        assert_eq!(lockstep.sim().unit(id).unwrap().target, Some(target));
    }

    #[test]
    #[should_panic(expected = "in order")]
    fn rejects_out_of_order_turns() {
        Lockstep::new(Sim::new(1, 1)).push_turn(turn(1, vec![]));
    }
}
