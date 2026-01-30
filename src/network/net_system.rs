use crate::network::net_manage::{Communication, Packet, TcpConnection, UdpConnection};
use crate::network::net_reconciliation::{StateTimeline, ObjectState, build_game_state, sequence_message, store_game_state};
use bevy::prelude::{info, Commands, Entity, Query, ResMut, Single};
use bincode::config;
use tokio::net::TcpStream;
use tokio::sync::mpsc::error::{TryRecvError, TrySendError};
use crate::network::net_message::{CTcpType, CUdpType, STcpType, SUdpType};

pub fn client_udp_net_receive(
    mut comm: ResMut<Communication>,
    mut connection: Single<&mut UdpConnection<CUdpType>>,
) {
    while !comm.udp_rx.is_empty() {
        match comm.udp_rx.try_recv() {
            Ok((bytes, addr)) => {
                match connection.socket {
                    Some(_) => {
                        connection.input_packet_buffer.push_back(Packet { bytes });
                    }
                    None => {
                        connection.socket = Some(addr);
                    }
                }
            }
            Err(_) => {}
        }
    }
}

pub fn client_udp_net_send(
    comm: ResMut<Communication>,
    mut connection: Single<&mut UdpConnection<CUdpType>>,
    mut reconcile_buffer: ResMut<StateTimeline>,
    mut commands: Commands,
) {
    if !connection.is_empty_messages() {
        sequence_message(
            &mut connection,
            &reconcile_buffer,
        );
        
        
        
        let encoded_message = match bincode::serde::encode_to_vec(connection.get_current_messages(), config::standard()) {
            Ok(m) => m,
            Err(e) => {
                println!("Couldn't encode UDP message: {:?}", e);
                return;
            }
        };

        if let Some(remote_socket) = &connection.socket {
            match comm.udp_tx.try_send(( encoded_message, *remote_socket )) {
                Ok(()) => {
                    connection.clear_messages();
                }
                Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Closed(_)) => {}
            }
        }
        
        reconcile_buffer.increment_sequence_num();
    }
}

pub fn client_tcp_net_receive(
    mut connection: Single<&mut TcpConnection<CTcpType>>,
    mut comm: ResMut<Communication>,
) {
    while !comm.tcp_rx.is_empty() {
        match comm.tcp_rx.try_recv() {
            Ok((bytes, stream)) => match connection.stream {
                Some(_) => {
                    connection.input_packet_buffer.push_back(Packet { bytes });
                }
                None => {
                    connection.stream = Some(stream);
                }
            },
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

pub fn client_tcp_net_send(
    comm: ResMut<Communication>,
    mut connection: Single<&mut TcpConnection<CTcpType>>
) {
    if !connection.is_empty_messages() {
        let encoded_message = match bincode::serde::encode_to_vec(connection.get_current_messages(), config::standard()) {
            Ok(m) => m,
            Err(e) => {
                println!("Couldn't encode TCP message: {:?}", e);
                return;
            }
        };

        if let Some(s) = &connection.stream {
            match comm.tcp_tx.try_send((encoded_message, s.clone())) {
                Ok(()) => {
                    connection.clear_messages();
                }
                Err(TrySendError::Full(_)) => return,
                Err(TrySendError::Closed(_)) => return,
            };
        }
    }
}

pub fn server_udp_net_receive(
    mut comm: ResMut<Communication>,
    mut connections: Query<&mut UdpConnection<SUdpType>>,
    mut commands: Commands,
) {
    while !comm.udp_rx.is_empty() {
        match comm.udp_rx.try_recv() {
            Ok((bytes, socket)) => {
                let c = connections
                    .iter_mut()
                    .find(|x| (x.socket.unwrap().ip() == socket.ip()) && (x.socket.unwrap().port() == socket.port()));

                match c {
                    Some(mut c) => {
                        c.input_packet_buffer.push_back(Packet {
                            bytes: bytes.clone(),
                        });
                    }
                    None => {
                        let mut conn = UdpConnection::<SUdpType>::new(Some(socket));
                        conn.input_packet_buffer.push_back(Packet {
                            bytes: bytes.clone(),
                        });
                        commands.spawn(conn);
                    }
                }
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

pub fn server_udp_net_send(comm: ResMut<Communication>, mut connections: Query<&mut UdpConnection<SUdpType>>) {
    for mut c in connections.iter_mut() {
        if c.is_empty_messages() {
            continue;
        }

        info!("Sending Message: {:?}", c.get_current_messages());

        let encoded_message =
            match bincode::serde::encode_to_vec(c.get_current_messages(), config::standard()) {
                Ok(m) => m,
                Err(e) => {
                    println!("Couldn't encode UDP message: {:?}", e);
                    continue;
                }
            };

        match comm.udp_tx.try_send((encoded_message.clone(), c.socket.unwrap())) {
            Ok(()) => {
                c.clear_messages();
            }
            Err(TrySendError::Full(_)) => break,
            Err(TrySendError::Closed(_)) => break,
        }
    }
}

pub fn server_tcp_net_receive(
    mut commands: Commands,
    mut connections: Query<&mut TcpConnection<STcpType>>,
    mut comm: ResMut<Communication>,
) {
    while !comm.tcp_rx.is_empty() {
        match comm.tcp_rx.try_recv() {
            Ok((bytes, stream)) => {
                let c = connections
                    .iter_mut()
                    .find(|x| same_stream(&*x.stream.clone().unwrap(), &*stream));

                match c {
                    Some(mut c) => {
                        c.input_packet_buffer.push_back(Packet {
                            bytes: bytes.clone(),
                        });
                    }
                    None => {
                        let mut conn = TcpConnection::<STcpType>::new(Some(stream));
                        conn.input_packet_buffer.push_back(Packet {
                            bytes: bytes.clone(),
                        });
                        commands.spawn(conn);
                    }
                }
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

pub fn server_tcp_net_send(comm: ResMut<Communication>, mut connections: Query<&mut TcpConnection<STcpType>>) {
    for mut c in connections.iter_mut() {
        if c.is_empty_messages() {
            continue;
        }

        let encoded_message =
            match bincode::serde::encode_to_vec(c.get_current_messages(), config::standard()) {
                Ok(m) => m,
                Err(e) => {
                    println!("Couldn't encode TCP message: {:?}", e);
                    continue;
                }
            };

        match comm
            .tcp_tx
            .try_send((encoded_message.clone(), c.stream.clone().unwrap()))
        {
            Ok(()) => {
                println!("OK");
                c.clear_messages();
            }
            Err(TrySendError::Full(_)) => break,
            Err(TrySendError::Closed(_)) => break,
        }
    }
}

fn same_stream(a: &TcpStream, b: &TcpStream) -> bool {
    a.peer_addr().ok() == b.peer_addr().ok() && a.local_addr().ok() == b.local_addr().ok()
}