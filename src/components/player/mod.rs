pub mod animation;
pub mod plugin;
mod input;

use crate::components::common::Id;
use crate::components::hud::Hud;
use crate::network::net_manage::UdpConnection;
use crate::network::net_message::{BitMask, NetworkMessage, SequenceNumber, CUdpType, SUdpType};
use crate::network::net_reconciliation::{StateTimeline, ObjectState, MISS_PREDICT_LIMIT, BUFFER_SIZE, get_next_sequence_num};
use bevy::asset::{AssetServer, Assets};
use bevy::input::ButtonInput;
use bevy::prelude::{error, info, warn, AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationPlayer, Camera, Capsule3d, ChildOf, Command, Component, Dir3, Entity, Gizmos, GlobalTransform, Handle, Local, Node, Reflect, Resource, Scene, SceneRoot, Single, Time, Val, Vec2, Vec3, World};
use bevy::prelude::{
    Camera3d, Commands, KeyCode, Mesh3d, MeshMaterial3d, Query, ReflectResource, Res, ResMut, Text, TextLayout, Transform, With,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::PI;
use std::time::Duration;
use avian3d::math::Quaternion;
use avian3d::prelude::{Collider, CollisionLayers, Friction, LayerMask, LinearVelocity, LockedAxes, Physics, PhysicsSchedule, Position, RigidBody, Rotation, ShapeCastConfig, Sleeping, SpatialQueryFilter, SpatialQueryPipeline};
use bevy::color::palettes::basic::{BLACK, PURPLE, WHITE};
use bevy::color::palettes::css::{RED, YELLOW};
use bevy::gltf::GltfAssetLabel;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseMotion};
use bevy::log::tracing_subscriber::fmt::time;
use bevy::math::EulerRot::YXZ;
use bevy::math::{Isometry3d, Quat};
use bevy::text::{FontSmoothing, TextFont};
use bevy::ui::PositionType;
use bevy::utils::default;
use crate::client_plugin::DefaultFont;
use crate::components::camera::{apply_player_camera_input, CameraInfo};
use crate::components::CollisionLayer;
use crate::components::player::animation::{AnimationState, PlayerAnimationState};
use crate::network::net_reconciliation::StateType::{Input, Player};

#[derive(Reflect, Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub enum MovementState {
    Idle,
    Walking,
    Running,
    Jumping,
    Falling,
}

#[derive(Resource, Default)]
pub struct PlayerInfo {
    pub current_player_id: Id,
    pub player_inputs: BitMask,
    pub mouse_delta: Vec2,
    pub accumulated_mouse_delta: Vec2,
    pub player_movement_state: HashSet<MovementState>,
}

#[derive(Component, Default, Debug, Copy, Clone)]
pub struct PredictedPlayerState {
    pub predicted_position: Vec3,
    pub predicted_linear_velocity: Vec3,
    pub predicted_yaw: f32,
    pub predicted_pitch: f32,
}

#[derive(Component)]
pub struct PlayerMarker;

#[derive(Serialize, Deserialize, Debug, Default, Copy, Clone, PartialEq)]
pub struct PlayerState {
    pub position: Vec3,
    pub linear_velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub animation_state: AnimationState
}

// pub struct ResimulatePlayer {
//     pub received_sequence_number: SequenceNumber,
//     pub object_states: Vec<ObjectState>,
// }

pub struct PlayerInput {
    pub keymask: BitMask,
    pub mouse_delta: Vec2,
}

#[derive(Component, Default)]
pub struct PendingInputs {
    pub buffer: VecDeque<PlayerInput>,
}

impl PlayerState {
    pub fn new(position: Vec3, linear_velocity: Vec3, yaw: f32, pitch: f32, animation_state: AnimationState) -> Self {
        Self {
            position,
            linear_velocity,
            yaw,
            pitch,
            animation_state
        }
    }
}

// impl ResimulatePlayer {
//     fn rollback_player(&self, world: &mut World) {
//         let rollback_player_state = {
//             let mut reconcile_buffer = world.resource_mut::<StateTimeline>();
// 
//             // Save frame state to buffer
//             reconcile_buffer
//                 .history
//                 .insert(self.received_sequence_number, self.object_states.clone());
// 
//             self.object_states
//                 .iter()
//                 .find_map(|object_state| {
//                         match object_state.0 {
//                             Player { player } => Some((player.position, player.linear_velocity, player.yaw, player.pitch)),
//                             _ => None
//                         }
//                     }
//                 )
//         };
// 
//         // Set transform to match historical frame state
//         let mut player = world.query_filtered::<(&mut Position, &mut LinearVelocity, &mut CameraInfo), With<PlayerMarker>>();
//         if let Some(mut p) = player.single_mut(world).ok() {
//             if let Some(pv) = rollback_player_state {
//                 info!("Rollback: position {:?}, linear_velocity {:?}", pv.0, pv.1);
// 
//                 p.0.0 = pv.0.into();
//                 p.1.0 = pv.1.into();
//                 p.2.yaw = pv.2;
//                 p.2.pitch = pv.3;
//             }
//         }
//     }
// 
//     fn resimulate_player(&self, world: &mut World) {
//         for i in self.received_sequence_number + 1.. {
//             // Extract input for this tick
//             let frame_input = {
//                 let reconcile_buffer = world.resource_mut::<StateTimeline>();
// 
//                 if !reconcile_buffer.seq_is_newer(i) {
//                     break;
//                 }
// 
//                 reconcile_buffer
//                     .history
//                     .get(&i)
//                     .and_then(|frame_state| {
//                         frame_state.iter().find_map(|object_state| match object_state.0 {
//                             Input { encoded_input , mouse_delta} => {
//                                 info!("Input Found");
//                                 Some((encoded_input, mouse_delta))
//                             },
//                             _ => None,
//                         })
//                     })
//             };
// 
//             if frame_input.is_none() {
//                 warn!("No input for frame {:?}", i);
//             }
// 
//             // Run the physics schedule
//             world.resource_mut::<Time<Physics>>().advance_by(Duration::from_secs_f64(1.0 / 60.0));
//             world.run_schedule(PhysicsSchedule);
// 
//             // Apply input
//             if let Some(fi) = frame_input {
//                 if let Some(mut player) = world
//                     .query_filtered::<(&Position, &mut LinearVelocity, &mut CameraInfo), With<PlayerMarker>>()
//                     .single_mut(world)
//                     .ok()
//                 {
//                     if fi.0 != 0 {
//                         apply_player_movement_input(fi.0, &mut player.1, &player.2.yaw);
//                     }
//                     apply_player_camera_input(fi.1, &mut player.2);
// 
//                     info!("Sequence {:?}: position {:?}, linear_velocity {:?}, key_mask {:?}", i, player.0, player.1, fi.0);
//                 }
//             }
// 
//             let new_player_info = {
//                 world
//                     .query_filtered::<(&Position, &LinearVelocity, &CameraInfo, &PlayerAnimationState), With<PlayerMarker>>()
//                     .single(world)
//                     .ok()
//                     .and_then(|p| Some((p.0.0, p.1.0, p.2.yaw, p.2.pitch, p.3.0)))
//             };
// 
//             // Save updated player state
//             let mut reconcile_buffer = world.resource_mut::<StateTimeline>();
// 
//             let index = if i == BUFFER_SIZE - 1 {
//                 0
//             } else {
//                 i + 1
//             };
// 
//             let fs = reconcile_buffer.history.get_mut(&index);
//             if fs.is_none() {
//                 info!("Couldn't find frame state for sequence {:?}", index);
//             }
// 
//             if let Some(frame_state) = fs {
//                 for object_state in frame_state.iter_mut() {
//                     match &mut object_state.0 {
//                         Player { player } => {
//                             if let Some(p) = new_player_info {
//                                 info!("Set state {:?}: position {:?}, linear_velocity {:?}", index, p.0, p.1);
// 
//                                 *player = PlayerState::new(
//                                     p.0.into(),
//                                     p.1.into(),
//                                     p.2,
//                                     p.3,
//                                     p.4
//                                 )
//                             }
//                         }
//                         _ => {}
//                     }
//                 }
//             }
//         }
//     }
// 
//     fn set_updated_player_state(&self, world: &mut World) {
//         let new_current_data = {
//             let reconcile_buffer = world.resource::<StateTimeline>();
// 
//             let index = if reconcile_buffer.sequence_counter == BUFFER_SIZE - 1 {
//                 0
//             } else {
//                 reconcile_buffer.sequence_counter + 1
//             };
// 
//             info!("Updated state sequence {:?}", index);
// 
//             reconcile_buffer
//                 .history
//                 .get(&(index))
//                 .and_then(|frame_state| {
//                     frame_state.iter().find_map(|object_state| {
//                         match object_state.0 {
//                             Player { player: player_state } => {
//                                 Some((player_state.position, player_state.linear_velocity, player_state.yaw, player_state.pitch))
//                             }
//                             _ => None
//                         }
//                     })
//                 })
//         };
// 
//         if new_current_data.is_none() {
//             error!("No updated player state found!");
//         }
// 
//         if let Some(ncd) = new_current_data {
//             if let Some(mut p) = world
//                 .query_filtered::<(&mut Position, &mut LinearVelocity, &mut CameraInfo), With<PlayerMarker>>()
//                 .single_mut(world)
//                 .ok()
//             {
//                 info!("Updated state: position {:?}, linear_velocity {:?}", ncd.0, ncd.1);
//                 p.0.0 = ncd.0.into();
//                 p.1.0 = ncd.1.into();
//                 p.2.yaw = ncd.2;
//                 p.2.pitch = ncd.3;
//             }
//         }
//     }
// }
// 
// impl Command for ResimulatePlayer {
//     fn apply(self, world: &mut World) -> () {
//         warn!("RESIMULATING");
//         self.rollback_player(world);
// 
//         self.resimulate_player(world);
// 
//         self.set_updated_player_state(world);
//     }
// }

pub fn set_player_id(
    player_info: &mut ResMut<PlayerInfo>,
    player_id: Id,
    reconcile_buffer: &mut StateTimeline
) {
    player_info.current_player_id = player_id;
    reconcile_buffer.history.clear()
}

const WALK_SPEED: f32 = 1.5;
const RUN_SPEED: f32 = 5.0;

const GRAVITY: f32 = 9.81;
const SKIN: f32 = 0.02;
const GROUND_SNAP: f32 = 0.15;
const GROUND_NORMAL_Y: f32 = 0.7;

fn apply_gravity(
    linear_velocity: &mut Vec3,
    time: &Time,
){
    linear_velocity.y -= GRAVITY * time.delta_secs();
}

fn apply_constraint_solver(
    spatial_query: &Res<SpatialQueryPipeline>,
    player_predicted_state: &mut PredictedPlayerState,
    collider: &Collider,
    // gizmos: &mut Gizmos,
    time: &Time,
) {
    // gizmos.ray(*position, *velocity, YELLOW);
    
    if let Some(hit) = spatial_query.cast_shape(
        collider,
        player_predicted_state.predicted_position,
        Quaternion::default(),
        Dir3::try_from(player_predicted_state.predicted_linear_velocity).unwrap(),
        &ShapeCastConfig{
            max_distance: player_predicted_state.predicted_linear_velocity.length() * time.delta_secs(),
            target_distance: 0.0,
            compute_contact_on_penetration: false,
            ignore_origin_penetration: true,
        },
        &SpatialQueryFilter::from_mask(!LayerMask::from(CollisionLayer::Player))
    ) {
        // gizmos.sphere(Isometry3d::new(*position + (Dir3::try_from(*velocity).unwrap() * hit.distance * time.delta_secs()), Quaternion::default()), 0.1, BLACK);
        // gizmos.sphere(Isometry3d::new(hit.point1, Quaternion::default()), 0.1, RED);
    
        let normal = hit.normal1;
        let into_surface = player_predicted_state.predicted_linear_velocity.dot(normal);
    
        if hit.normal1.y > 0.7 {
            // SNAP TO GROUND
            player_predicted_state.predicted_linear_velocity.y = 0.0;
            return;
        }
    
        if into_surface < 0.0 {
            player_predicted_state.predicted_linear_velocity -= normal * into_surface;
        }
    
        let max_move = (hit.distance - 0.02).max(0.0);
        if player_predicted_state.predicted_linear_velocity.length() > max_move {
            player_predicted_state.predicted_linear_velocity = player_predicted_state.predicted_linear_velocity.normalize() * max_move;
        }
    }
}

fn apply_player_movement_input(
    encoded_input: BitMask,
    linear_velocity: &mut Vec3,
    yaw: &f32,
) {
    let mut vector = Vec3::ZERO;

    if encoded_input & 1 > 0 {
        vector.z -= 1.0;
    }
    if encoded_input & 2 > 0 {
        vector.z += 1.0;
    }
    if encoded_input & 4 > 0 {
        vector.x += 1.0;
    }
    if encoded_input & 8 > 0 {
        vector.x -= 1.0;
    }
    if encoded_input & 16 > 0 {
        linear_velocity.y += 1.0;
    }

    let normalized_rotated_velocity = Quat::from_euler(YXZ, *yaw, 0.0, 0.0).mul_vec3(vector.normalize_or_zero());
    
    linear_velocity.x = normalized_rotated_velocity.x * WALK_SPEED;
    linear_velocity.z = normalized_rotated_velocity.z * WALK_SPEED;
}

pub fn player_controller(
    mut player_info: ResMut<PlayerInfo>,
    mut players: Query<(&Id, &mut PredictedPlayerState, &mut CameraInfo, &mut PlayerAnimationState, &Collider), With<PlayerMarker>>,
    spatial_query: Res<SpatialQueryPipeline>,
    mut hud: Query<&mut Text, With<Hud>>,
    mut connection: Single<&mut UdpConnection<CUdpType>>,
    state_timeline: Res<StateTimeline>,
    time: Res<Time>,
    mut gizmos: Gizmos,
    mut commands: Commands,
) {
    if connection.socket.is_some() {
        for (id, mut player_predicted_state, mut camera_info, mut player_anim_state, collider) in players.iter_mut() {
            
            apply_gravity(&mut player_predicted_state.predicted_linear_velocity, &time);
            
            if player_info.current_player_id == *id {
                if player_info.player_inputs != 0 {
                    player_anim_state.0 = AnimationState::Walking;
                    apply_player_movement_input(player_info.player_inputs, &mut player_predicted_state.predicted_linear_velocity, &camera_info.yaw);
                } else {
                    player_anim_state.0 = AnimationState::Idle;
                    info!("No input");
                }

                if let Some(mut h) = hud.single_mut().ok() {
                    h.clear();
                    h.push_str(&format!(
                        "x: {:?}\ny: {:?}\nz: {:?}\nping: {:?}\n{:?}",
                        player_predicted_state.predicted_position.x, player_predicted_state.predicted_position.y, player_predicted_state.predicted_position.z, connection.ping, player_info.current_player_id
                    ));
                }

                apply_constraint_solver(&spatial_query, &mut player_predicted_state, collider, &time);
                
                commands.spawn(ObjectState(Player { player: PlayerState::new(player_predicted_state.predicted_position, player_predicted_state.predicted_linear_velocity, camera_info.yaw, camera_info.pitch, player_anim_state.0) }));
                commands.spawn(ObjectState(Input { encoded_input: player_info.player_inputs, mouse_delta: player_info.accumulated_mouse_delta - player_info.mouse_delta }));
            }
        }

        connection.add_message(NetworkMessage(CUdpType::PlayerId {id: player_info.current_player_id}));
        
        connection.add_message(NetworkMessage(CUdpType::Input {
            keymask: player_info.player_inputs,
            mouse_delta: player_info.accumulated_mouse_delta - player_info.mouse_delta,
        }));

        player_info.accumulated_mouse_delta = Vec2::new(0.0, 0.0);
    }
}

pub fn resimulate_player(
    state_timeline: &mut ResMut<StateTimeline>,
    received_seq_num: SequenceNumber,
    mut predicted_player_state: &mut PredictedPlayerState,
    collider: &Collider,
    spatial_query: &Res<SpatialQueryPipeline>,
    time: &Res<Time>
) {
    let mut circular_index;

    for i in received_seq_num + 1.. {
        circular_index = i % BUFFER_SIZE;

        if circular_index == (state_timeline.sequence_counter + 1) % BUFFER_SIZE {
            break
        }

        let mut player_state = None;
        let mut input_state = None;

        if let Some(reconcile_objects) = state_timeline.history.get(&received_seq_num) {
            for r in reconcile_objects {
                match r.0 {
                    Player { player } => player_state = Some(player),
                    Input { encoded_input, mouse_delta } =>  input_state = Some((encoded_input, mouse_delta))
                }
            }
        }
        
        if let Some(mut player) = player_state {
            apply_gravity(&mut predicted_player_state.predicted_linear_velocity, &time);
            if let Some((input_mask, rotation)) = input_state {
                if input_mask != 0 {
                    apply_player_movement_input(input_mask, &mut player.linear_velocity, &player.yaw);
                }
                apply_player_camera_input(rotation.into(), &mut predicted_player_state);
            }
            apply_constraint_solver(&spatial_query, &mut predicted_player_state, &collider, &time);
        }

        if let Some(frame_state) = state_timeline.history.get_mut(&circular_index) {
            for object_state in frame_state.iter_mut() {
                match &mut object_state.0 {
                    Player { player } => {
                        *player = PlayerState::new(
                            predicted_player_state.predicted_position,
                            predicted_player_state.predicted_linear_velocity,
                            predicted_player_state.predicted_yaw,
                            predicted_player_state.predicted_pitch,
                            AnimationState::Idle
                        )
                    },
                    _ => {}
                }
            }
        }
        
    }
}

pub fn reconcile_player(
    commands: &mut Commands,
    gizmos: &mut Gizmos,
    received_seq_num: SequenceNumber,
    server_players: &HashMap<Id, PlayerState>,
    client_players: &mut Query<(&mut Transform, &Id, Entity, &CameraInfo, &mut PlayerAnimationState, &mut PredictedPlayerState, &Collider), With<PlayerMarker>>,
    player_info: &Res<PlayerInfo>,
    mut state_timeline: &mut ResMut<StateTimeline>,
    spatial_query: &Res<SpatialQueryPipeline>,
    time: &Res<Time>
) {
    let server_player_state = server_players.get(&player_info.current_player_id);

    let client_player_state = if let Some(reconcile_objects) = state_timeline.history.get(&received_seq_num) {
        let mut found_state = None;
        for r in reconcile_objects {
            match r.0 {
                Player { player } => {
                    found_state = Some(player);
                    break;
                },
                _ => {}
            }
        }
        found_state
    } else {
        None
    };
        
    for (_, id, _, _, _, mut predicted_player_state, collider) in client_players.iter_mut() {
        if player_info.current_player_id == *id
            && server_player_state.is_some()
            && client_player_state.is_some()
        {
            let sps = *server_player_state.unwrap();
            let cps = client_player_state.unwrap();

            gizmos.cube(
                Transform::from_xyz(sps.position.x, sps.position.y, sps.position.z)
                    .with_scale(Vec3::splat(1.1))
                    .with_rotation(Quat::from_euler(YXZ, sps.yaw,0.0,0.0)),
                BLACK
            );

            gizmos.cube(
                Transform::from_xyz(cps.position.x, cps.position.y, cps.position.z).with_rotation(Quat::from_euler(YXZ, cps.yaw,0.0,0.0)),
                PURPLE
            );

            if !sps.eq(&cps) {
                if state_timeline.miss_predict_counter >= MISS_PREDICT_LIMIT - 1 {
                    warn!("RECONCILED");
                    info!("current sequence: {:?}, recieved sequence: {:?}", state_timeline.sequence_counter, received_seq_num);
                    info!("client: {:?}, server: {:?}", cps.position, sps.position);


                    if let Some(new_frame_state) = state_timeline.history.get_mut(&received_seq_num) {
                        for entity_state in new_frame_state.iter_mut() {
                            match &mut entity_state.0 {
                                Player { player } => {
                                    // Sets state back to received server state to prepare for resimulation
                                    *player = PlayerState::new(sps.position, sps.linear_velocity, sps.yaw, sps.pitch, sps.animation_state);
                                }
                                _ => {}
                            }
                        }

                        // predicted_player_state.predicted_position = sps.position;
                        // predicted_player_state.predicted_linear_velocity = sps.linear_velocity;
                        // predicted_player_state.predicted_pitch = sps.pitch;
                        // predicted_player_state.predicted_yaw = sps.yaw;

                        // resimulate_player(&mut state_timeline, received_seq_num, &mut predicted_player_state, &collider, &spatial_query, &time);

                        state_timeline.miss_predict_counter = 0;
                    }
                } else {
                    state_timeline.miss_predict_counter += 1;
                }
            }
        }
    }
}

// pub fn spawn_players(
//     mut commands: Commands,
//     mut meshes: ResMut<Assets<Mesh>>,
//     mut materials: ResMut<Assets<StandardMaterial>>,
//     mut net_message: ResMut<NetworkMessages>,
// ) {
//     let res = &mut net_message.udp_messages;
//     for m in res {
//         match &m.0 {
//             UDP::Spawn { player_uid } => {
//                 println!("Spawning player {:?}", player_uid);
//
//                 let mesh = Mesh::from(Sphere::default());
//                 for p in player_uid {
//                     commands.spawn((
//                         Mesh3d(meshes.add(mesh.clone())),
//                         MeshMaterial3d(materials.add(StandardMaterial::from(Color::WHITE))),
//                         Transform::from_xyz(0.0, 0.0, 0.0).with_scale(Vec3::splat(128.)),
//                         Id(p.0),
//                     ));
//                 }
//             }
//             _ => {}
//         }
//     }
// }

pub fn update_players(
    commands: &mut Commands,
    default_font: &Res<DefaultFont>,
    asset_server: &Res<AssetServer>,
    server_players: &HashMap<Id, PlayerState>,
    client_players: &mut Query<(&mut Transform, &Id, Entity, &CameraInfo, &mut PlayerAnimationState, &mut PredictedPlayerState, &Collider), With<PlayerMarker>>,
    info: &Res<PlayerInfo>,
) {
    let mut existing_players = HashSet::new();

    for (mut transform, id, entity, _, mut anim_state, _, _) in client_players.iter_mut() {
        existing_players.insert(id);

        let player = match server_players.get(id) {
            Some(p) => p,
            None => continue,
        };

        if *id != info.current_player_id {
            commands.entity(entity).remove::<LinearVelocity>();
            commands.entity(entity).remove::<RigidBody>();
            commands.entity(entity).remove::<LockedAxes>();
            commands.entity(entity).remove::<Friction>();
            commands.entity(entity).remove::<Sleeping>();
            commands.entity(entity).remove::<CollisionLayers>();

            commands.entity(entity).insert(CollisionLayers::new(CollisionLayer::Enemy, [LayerMask::ALL]));

            transform.translation = player.position.into();
            transform.rotation = Quat::from_euler(YXZ, player.yaw, 0.0, 0.0);
            anim_state.0 = player.animation_state;
        }
    }

    // Spawns players if they do not exist
    for p in server_players.iter() {
        if !existing_players.contains(p.0) {
            println!("{:?}", p.1.position);

            let player = commands.spawn((
                RigidBody::Kinematic,
                Collider::capsule(0.5, 1.0),
                LockedAxes::new()
                    .lock_rotation_x()
                    .lock_rotation_y()
                    .lock_rotation_z(),
                Position::from_xyz(
                    p.1.position.x,
                    p.1.position.y,
                    p.1.position.z
                ),
                CollisionLayers::new(CollisionLayer::Player, [LayerMask::ALL]),
                CameraInfo::default(),
                PlayerAnimationState(AnimationState::Idle),
                PredictedPlayerState {
                    predicted_position: p.1.position,
                    predicted_linear_velocity: p.1.linear_velocity,
                    predicted_yaw: p.1.yaw,
                    predicted_pitch: p.1.pitch,
                },
                *p.0,
                PlayerMarker
            )).with_children( |parent| {
                parent.spawn((
                    SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("meshes\\player.glb"))),
                    Transform::from_xyz(0.0, -1.0, 0.0).with_rotation(Quat::from_euler(YXZ, PI, 0.0, 0.0)).with_scale(bevy::math::Vec3::splat(0.15)),
                ));
            }).id();

            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                PlayerLabel(player)
            )).with_children(|parent| {
                parent.spawn((
                    Text::new(p.0.0.to_string()),
                    TextFont{
                        font: default_font.0.clone(),
                        font_size: 20.0,
                        weight: Default::default(),
                        font_smoothing: FontSmoothing::None,
                        font_features: Default::default(),
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        bottom: Val::ZERO,
                        ..default()
                    },
                    TextLayout::default().with_no_wrap(),
                ));
            });
        }
    }
}

#[derive(Component)]
pub struct PlayerLabel(Entity);

pub fn update_label_pos(
    mut labels: Query<(Entity, &mut Node, &PlayerLabel)>,
    players: Query<&GlobalTransform>,
    camera3d: Query<(&mut Camera, &GlobalTransform), With<Camera3d>>,
    mut commands: Commands
) {
    for (entity, mut node, label) in &mut labels {
        if let Some(world_position) = players.get(label.0).ok() {
            let pos = world_position.translation() + Vec3::Y;
            
            let (camera, camera_transform) = camera3d.single().unwrap();

            let viewport_position = match camera.world_to_viewport(camera_transform, pos) {
                Ok(v) => v,
                Err(e) => { continue; },
            };

            node.top = Val::Px(viewport_position.y);
            node.left = Val::Px(viewport_position.x);
        } else {
            commands.entity(entity).despawn();
        }
    }
}

//TODO: Add after reconciliation check
pub fn update_player_kinematics(
    mut player_query: Query<(&mut Position, &mut Rotation, &mut LinearVelocity, &PredictedPlayerState), With<PlayerMarker>>,
){
    for (mut position, mut rotation, mut linear_velocity, predicted_state) in player_query.iter_mut() {
        position.0 = predicted_state.predicted_position;
        linear_velocity.0 = predicted_state.predicted_linear_velocity;
        rotation.0 = Quat::from_euler(YXZ, predicted_state.predicted_yaw, 0.0, 0.0);
    }
}