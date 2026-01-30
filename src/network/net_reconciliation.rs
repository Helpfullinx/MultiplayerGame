use crate::components::player::PlayerState;
use crate::network::net_message::{BitMask, NetworkMessage, SequenceNumber, CUdpType};
use bevy::prelude::{info, Commands, Component, Entity, Query, ResMut, Resource, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::network::net_manage::UdpConnection;

pub const BUFFER_SIZE: u16 = 1024;

// Used to determine how many miss predicts happen before rollback and resimulation
pub const MISS_PREDICT_LIMIT: u16 = 20;

#[derive(Component, Serialize, Deserialize, Clone, Debug)]
pub struct ObjectState(pub StateType);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum StateType {
    Player { player: PlayerState },
    Input { encoded_input: BitMask, mouse_delta: Vec2 }
}

/// Holds a circular buffer of the last BUFFER_SIZE amount of game states
#[derive(Resource)]
pub struct StateTimeline {
    pub history: HashMap<SequenceNumber, Vec<ObjectState>>,
    pub sequence_counter: SequenceNumber,
    pub miss_predict_counter: u16
}

impl StateTimeline {
    /// Use this function to increment the sequence number instead of directly accessing sequence_counter
    /// variable due to buffer being circular
    pub fn increment_sequence_num(self: &mut Self) {
        if self.sequence_counter >= BUFFER_SIZE - 1 {
            self.sequence_counter = 0;
        } else {
            self.sequence_counter = self.sequence_counter + 1;
        }
    }

    pub fn seq_is_newer(self: &Self, rhs: SequenceNumber) -> bool {
        let diff = (self.sequence_counter.wrapping_sub(rhs)) % BUFFER_SIZE;
        diff == 0 || diff < BUFFER_SIZE / 2
    }
}

pub fn get_next_sequence_num(sequence_number: &SequenceNumber) -> SequenceNumber {
    if *sequence_number >= BUFFER_SIZE - 1 {
         0
    } else {
        sequence_number + 1
    }
}

pub fn build_game_state(
    object_states: &mut Query<(Entity, &ObjectState)>,
    commands: &mut Commands,
) -> Vec<ObjectState> {
    let mut game_state = Vec::new();
    for n in object_states.iter_mut() {
        game_state.push(n.1.clone());
        commands.entity(n.0).despawn();
    }
    
    game_state
}

pub fn sequence_message(
    connection: &mut UdpConnection<CUdpType>,
    timeline: &StateTimeline,
) {
    let current_sequence = timeline.sequence_counter;

    connection.add_message(NetworkMessage(CUdpType::Sequence {
        sequence_number: current_sequence,
    }));
}

pub fn store_game_state(
    game_state: Vec<ObjectState>,
    timeline: &mut ResMut<StateTimeline>,
) {
    // info!("STORING GAME STATE: {:?}", reconcile_buffer.sequence_counter);
    let current_sequence = timeline.sequence_counter;
    
    timeline
        .history
        .insert(current_sequence, game_state);
}

pub fn game_state_system(
    mut object_states: Query<(Entity, &ObjectState)>,
    mut timeline: ResMut<StateTimeline>,
    mut commands: Commands
) {
    let game_state = build_game_state(&mut object_states, &mut commands);


    store_game_state(
        game_state,
        &mut timeline
    );
}