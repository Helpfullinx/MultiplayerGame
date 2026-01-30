use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use avian3d::parry::na::DimAdd;
use bevy::app::{App, Plugin};
use bevy::prelude::{Commands, FixedPostUpdate, FixedPreUpdate, FixedUpdate, IntoScheduleConfigs, PreStartup, Res, Resource};
use bevy_inspector_egui::egui::TextBuffer;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use crate::components::chat::send_chat_to_all_connections;
use crate::components::player::PlayerState;
use crate::network;
use crate::network::net_manage::{start_tcp_connection, start_tcp_listener, start_udp_connection, start_udp_listener, Communication, TcpConnection, UdpConnection};
use crate::network::net_message::SequenceNumber;
use crate::network::net_reconciliation::{game_state_system, ObjectState, StateTimeline, BUFFER_SIZE};
use crate::network::net_reconciliation::StateType::{Input, Player};
use crate::network::net_system::{client_tcp_net_receive, client_tcp_net_send, server_tcp_net_receive, client_udp_net_receive, client_udp_net_send, server_udp_net_receive, server_udp_net_send, server_tcp_net_send};
use crate::network::net_tasks::{add_ping_message, build_connection_messages, client_handle_tcp_message, client_handle_udp_message, server_handle_tcp_message, server_handle_udp_message};

#[derive(Resource)]
pub struct RemoteAddress(pub String);

#[derive(Resource, Clone, Copy)]
pub struct NetworkConfig {
    pub host_type: HostType,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HostType {
    Client,
    Server,
}

pub struct NetworkPlugin {
    pub config: NetworkConfig,
}

impl NetworkPlugin {
    pub fn new(config: NetworkConfig) -> Self {
        Self { config }
    }
}

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        // let mut initial_history: HashMap<SequenceNumber, Vec<ObjectState>> = HashMap::new();
        // for i in 0..BUFFER_SIZE {
        //     let mut object_states = Vec::new();
        //     object_states.push(ObjectState(PlayerState{ player: Player::default() }));
        //     object_states.push(ObjectState(InputState { encoded_input: 0, mouse_delta: Default::default() }));
        //     initial_history.insert(i, object_states);
        // }

        match self.config.host_type {
            HostType::Client => {
                app
                    .add_plugins(TokioTasksPlugin::default())
                    .insert_resource(StateTimeline {
                        history: HashMap::new(),
                        sequence_counter: 0,
                        miss_predict_counter: 0,
                    })
                    .insert_resource(self.config.clone())
                    .add_systems(PreStartup, setup_communications)
                    .add_systems(
                        FixedPreUpdate,
                        (
                            client_udp_net_receive,
                            client_tcp_net_receive,
                            client_handle_udp_message.after(client_udp_net_receive),
                            client_handle_tcp_message.after(client_tcp_net_receive),
                            add_ping_message.after(client_handle_udp_message),
                        )
                    )
                    .add_systems(
                        FixedPostUpdate,
                        (
                            game_state_system,
                            client_udp_net_send,
                            client_tcp_net_send
                        ).chain()
                    );
            }
            HostType::Server => {
                app.add_plugins(TokioTasksPlugin::default())
                    .add_systems(PreStartup, setup_communications)
                    .add_systems(
                        FixedPreUpdate,
                        (
                            server_udp_net_receive,
                            server_tcp_net_receive,
                            server_handle_udp_message.after(server_udp_net_receive),
                            server_handle_tcp_message.after(server_tcp_net_receive),
                        ),
                    )
                    .add_systems(
                        FixedPostUpdate,
                        (
                            send_chat_to_all_connections,
                            build_connection_messages,
                            server_udp_net_send.after(build_connection_messages),
                            server_tcp_net_send.after(send_chat_to_all_connections),
                        ),
                    );
            }
        }


    }
}

fn setup_communications(
    mut commands: Commands,
    network_config: Res<NetworkConfig>,
    remote_addr_resource: Res<RemoteAddress>,
    runtime: Res<TokioTasksRuntime>
) {
    println!("Setting up communications...");
    let (udp_send_tx, udp_send_rx) = mpsc::channel::<(Vec<u8>, SocketAddr)>(1_000);
    let (udp_receive_tx, udp_receive_rx) = mpsc::channel::<(Vec<u8>, SocketAddr)>(1_000);
    let (tcp_send_tx, tcp_send_rx) = mpsc::channel::<(Vec<u8>, Arc<TcpStream>)>(1_000);
    let (tcp_receive_tx, tcp_receive_rx) = mpsc::channel::<(Vec<u8>, Arc<TcpStream>)>(1_000);

    match network_config.host_type {
        HostType::Client => {
            let remote_string = remote_addr_resource.0.clone();
            runtime.spawn_background_task(|_| async move {
                println!("starting communication");
                println!("remote address: {}", remote_string);
                let remote_addr = SocketAddr::from_str(format!("{}:4444", remote_string).as_str())
                    .ok()
                    .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 4444)));

                start_tcp_connection(remote_addr, tcp_send_rx, tcp_receive_tx).await.unwrap();
                start_udp_connection(remote_addr, udp_send_rx, udp_receive_tx, 1).await.unwrap();
            });
        }
        HostType::Server => {
            runtime.spawn_background_task(|_| async move {
                let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 4444);
                println!("Server starting; listening on 0.0.0.0:4444...");

                start_tcp_listener(addr, tcp_send_rx, tcp_receive_tx)
                    .await
                    .unwrap();
                start_udp_listener(addr, udp_send_rx, udp_receive_tx, 8)
                    .await
                    .unwrap();
            });
        }
    }

    commands.insert_resource(
        Communication::new(
            udp_send_tx,
            udp_receive_rx,
            tcp_send_tx,
            tcp_receive_rx,
        )
    );
}

