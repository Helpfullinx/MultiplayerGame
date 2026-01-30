use std::cmp::min;
use std::collections::HashMap;
use std::time::SystemTime;
use avian3d::prelude::{Collider, LinearVelocity, Position, Rotation, SpatialQueryPipeline};
use crate::components::chat::{Chat, client_add_chat_message, server_add_chat_message};
use crate::components::common::Id;
use crate::components::player::{PlayerInfo, reconcile_player, set_player_id, update_players, PlayerMarker, PredictedPlayerState, PlayerState, PendingInputs, PlayerInput};
use crate::network::net_manage::{TcpConnection, UdpConnection};
use crate::network::net_message::{BitMask, CTcpType, CUdpType, NetworkMessage, STcpType, SUdpType, SequenceNumber};
use crate::network::net_reconciliation::StateTimeline;
use bevy::asset::{AssetServer, Assets};
use bevy::pbr::StandardMaterial;
use bevy::prelude::{info, warn, AnimationGraph, Commands, Entity, Gizmos, Mesh, Quat, Query, Res, ResMut, Single, Time, Transform, Vec2, With};
use bincode::config;
use crate::client_plugin::DefaultFont;
use crate::components::camera::CameraInfo;
use crate::components::lobby::handle_join;
use crate::components::player::animation::PlayerAnimationState;
use crate::network::net_message::CUdpType::{Input, Ping, PlayerId, Sequence};
use crate::network::net_message::SUdpType::Pong;

const MESSAGE_PER_TICK_MAX: usize = 20;

struct MessageBuffer {
    sequence_number: i32,
    player_id: Option<Id>,
    keymask: BitMask,
    mouse_delta: Vec2,
    pong_message: Option<SUdpType>,
}

pub fn client_handle_udp_message(
    mut gizmos: Gizmos,
    mut connection: Single<&mut UdpConnection<CUdpType>>,
    mut client_players: Query<(&mut Transform, &Id, Entity, &CameraInfo, &mut PlayerAnimationState, &mut PredictedPlayerState, &Collider), With<PlayerMarker>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut animation_graphs: ResMut<Assets<AnimationGraph>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut reconcile_buffer: ResMut<StateTimeline>,
    default_font: Res<DefaultFont>,
    player_info: Res<PlayerInfo>,
    spatial_query: Res<SpatialQueryPipeline>,
    time: Res<Time>,
) {
    while let Some(p) = connection.input_packet_buffer.pop_front() {
        let decoded_message: (Vec<SUdpType>, usize) = match bincode::serde::decode_from_slice(&p.bytes, config::standard()) {
            Ok(m) => m,
            Err(e) => {
                println!("Couldn't decode UDP message: {:?}", e);
                continue;
            }
        };

        let mut seq_num = None;

        for m in decoded_message.0.iter() {
            match m {
                SUdpType::Sequence { sequence_number } => {
                    seq_num = Some(sequence_number);
                }
                _ => {}
            }
        }

        if seq_num.is_none() {
            println!("No sequence number given");
            continue;
        }
        
        for m in decoded_message.0.iter() {
            match m {
                SUdpType::Players { players } => {
                    reconcile_player(
                        &mut commands,
                        &mut gizmos,
                        *seq_num.unwrap(),
                        &players,
                        &mut client_players,
                        &player_info,
                        &mut reconcile_buffer,
                        &spatial_query,
                        &time
                    );
                    update_players(
                        &mut commands,
                        &default_font,
                        &asset_server,
                        &players,
                        &mut client_players,
                        &player_info,
                    );
                },
                SUdpType::Pong { initiation_time, server_received_time } => {
                    let time_now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as u32;
                    let rtt = time_now - *initiation_time;
                    
                    connection.ping = rtt;
                }
                SUdpType::Sequence { .. } => {}
            }
        }
    }
}

pub fn client_handle_tcp_message(
    mut player_info: ResMut<PlayerInfo>,
    mut chat: Query<&mut Chat>,
    mut connection: Single<&mut TcpConnection<CTcpType>>,
    mut reconcile_buffer: ResMut<StateTimeline>
) {
    while let Some(p) = connection.input_packet_buffer.pop_front() {
        let mut decoded_message: (Vec<STcpType>, usize) = match bincode::serde::decode_from_slice(&p.bytes, config::standard()) {
            Ok(m) => m,
            Err(e) => {
                println!("Couldn't decode TCP message: {:?}", e);
                continue;
            }
        };
        
        for m in decoded_message.0.iter_mut() {
            match m {
                STcpType::Chat { messages } => {
                    client_add_chat_message(messages, &mut chat);
                },
                STcpType::PlayerId { player_uid } => {
                    set_player_id(&mut player_info, *player_uid, &mut reconcile_buffer);
                }
            }
        }
    }
}

pub fn server_handle_udp_message(
    mut connections: Query<&mut UdpConnection<SUdpType>>,
    mut players: Query<
        (&Id, &mut LinearVelocity, &mut Rotation, &mut CameraInfo, &mut PlayerAnimationState, &Position, &mut PendingInputs),
        With<PlayerMarker>,
    >,
) {
    for mut c in connections.iter_mut() {
        if c.input_packet_buffer.is_empty() {
            continue;
        }

        let mut current_message = MessageBuffer {
            sequence_number: -1,
            player_id: None,
            keymask: 0,
            mouse_delta: Vec2::new(0.0,0.0),
            pong_message: None,
        };

        for _ in 0..min(MESSAGE_PER_TICK_MAX, c.input_packet_buffer.len()) {
            match c.input_packet_buffer.pop_front() {
                Some(p) => {
                    let decoded_message: (Vec<CUdpType>, usize) =
                        match bincode::serde::decode_from_slice(&p.bytes, config::standard()) {
                            Ok(m) => m,
                            Err(e) => {
                                println!("Couldn't decode UDP message: {:?}", e);
                                continue;
                            }
                        };

                    warn!("Received UDP message: {:?}", decoded_message);

                    for m in decoded_message.0.iter() {
                        match m {
                            PlayerId {id} => {
                                current_message.player_id = Some(id.clone())
                            }
                            Sequence { sequence_number } => {
                                info!("Received Sequence: {:?}", sequence_number);
                                current_message.sequence_number = sequence_number.clone() as i32;
                            }
                            Input {
                                keymask,
                                mouse_delta,
                            } => {
                                current_message.keymask = *keymask;
                                current_message.mouse_delta = *mouse_delta;

                                info!("Received Keymask: {:?}", keymask);
                            },
                            Ping { start_time: initiation_time, last_rtt } => {
                                c.ping = *last_rtt;
                                let time_now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis();
                                current_message.pong_message = Some(Pong{ initiation_time: *initiation_time, server_received_time: time_now as u32 });
                            }
                        }
                    }

                    if let Some(id) = current_message.player_id {
                        for mut p in players.iter_mut() {
                            if id == *p.0 {
                                p.6.buffer.push_back(PlayerInput{keymask: current_message.keymask, mouse_delta: current_message.mouse_delta})
                            }
                        }
                    }
                }
                None => {}
            }
        }

        if current_message.sequence_number != -1 {
            info!("Sequence Added: {:?}", current_message.sequence_number);
            c.add_message(NetworkMessage(SUdpType::Sequence {
                sequence_number: current_message.sequence_number as SequenceNumber,
            }));
        };

        if let Some(pong) = current_message.pong_message {
            c.add_message(NetworkMessage(pong));
        }

        if !c.input_packet_buffer.is_empty() {
            info!("Clearing input buffer");
            c.input_packet_buffer.clear()
        }
    }
}

pub fn server_handle_tcp_message(
    mut chat: Query<&mut Chat>,
    mut connections: Query<&mut TcpConnection<STcpType>>,
    mut commands: Commands,
) {
    for mut c in connections.iter_mut() {
        if c.input_packet_buffer.is_empty() {
            continue;
        }

        for _ in 0..min(MESSAGE_PER_TICK_MAX, c.input_packet_buffer.len()) {
            match c.input_packet_buffer.pop_front() {
                Some(p) => {
                    let mut decoded_message: (Vec<CTcpType>, usize) =
                        match bincode::serde::decode_from_slice(&p.bytes, config::standard()) {
                            Ok(m) => m,
                            Err(e) => {
                                println!("Couldn't decode TCP message: {:?}", e);
                                continue;
                            }
                        };

                    for m in decoded_message.0.iter_mut() {
                        match m {
                            CTcpType::ChatMessage { player_id, message } => {
                                server_add_chat_message((*player_id, message.clone()), &mut chat);
                            }
                            CTcpType::Join { lobby_id } => {
                                handle_join(*lobby_id, &mut c, &mut commands);
                            }
                            _ => {}
                        }
                    }
                }
                None => {}
            }
        }
    }
}

pub fn build_connection_messages(
    mut connections: Query<&mut UdpConnection<SUdpType>>,
    players: Query<
        (&Id, &Position, &LinearVelocity, &CameraInfo, &PlayerAnimationState),
        With<PlayerMarker>, /*, Changed<Transform>*/
    >,
) {
    let changed_players: HashMap<Id, PlayerState> = players
        .iter()
        .map(|(i, p, l, c, pas)| {
            let player = PlayerState::new(
                p.0.into(),
                p.0.into(),
                c.yaw,
                c.pitch,
                pas.0
            );

            (*i, player)
        })
        .collect();

    for mut c in connections.iter_mut() {
        if c.contains_message_type(SUdpType::Sequence { sequence_number: 0 }) {
            c.add_message(NetworkMessage(SUdpType::Players {
                players: changed_players.clone(),
            }));
        }
    }
}

pub fn add_ping_message(
    mut connection: Single<&mut UdpConnection<CUdpType>>
) {
    let time_now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as u32;
    let last_rtt = connection.ping;
    connection.add_message(NetworkMessage(Ping{ start_time: time_now, last_rtt }))
}
