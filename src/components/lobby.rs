use rand::Rng;
use avian3d::prelude::{Collider, Friction, LinearVelocity, LockedAxes, RigidBody};
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::{Commands, KeyCode, MessageReader, Single, Transform};
use crate::components::camera::CameraInfo;
use crate::components::common::Id;
use crate::components::player::animation::{AnimationState, PlayerAnimationState};
use crate::components::player::{PendingInputs, PlayerMarker};
use crate::network::net_manage::TcpConnection;
use crate::network::net_message::{CTcpType, NetworkMessage, STcpType};

const LOBBY_ID: u32 = 1;
pub fn join_lobby(
    mut keyboard_input: MessageReader<KeyboardInput>,
    mut connection: Single<&mut TcpConnection<CTcpType>>,
) {
    for k in keyboard_input.read() {
        if k.state == ButtonState::Released {
            continue;
        };

        match k.key_code {
            KeyCode::KeyJ => {
                if connection.stream.is_some() {
                    connection.add_message(NetworkMessage(CTcpType::Join { lobby_id: Id(LOBBY_ID) }));
                }
            }
            _ => {}
        }
    }
}

pub fn handle_join(lobby_id: Id, connection: &mut TcpConnection<STcpType>, commands: &mut Commands) {
    println!("Trying to join lobby: {:?}", lobby_id);
    // Generate an ID

    let player_id = generate_random_u32();

    println!("Player joined: {:?}", player_id);

    commands.spawn((
        RigidBody::Kinematic,
        Collider::capsule(0.5, 1.0),
        Friction::new(1.0),
        LinearVelocity::default(),
        LockedAxes::new()
            .lock_rotation_x()
            .lock_rotation_y()
            .lock_rotation_z(),
        Transform::from_xyz(0.0, 3.0, 0.0),
        CameraInfo {
            yaw: 0.0,
            pitch: 0.0,
        },
        PlayerAnimationState(AnimationState::Idle),
        PendingInputs::default(),
        Id(player_id),
        PlayerMarker,
    ));

    connection.add_message(NetworkMessage(STcpType::PlayerId {
        player_uid: Id(player_id),
    }));
}

fn generate_random_u32() -> u32 {
    let mut rng = rand::rng();
    rng.random::<u32>()
}