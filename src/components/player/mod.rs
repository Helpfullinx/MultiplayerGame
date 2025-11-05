pub mod animation;
pub mod plugin;
mod input;

use crate::components::common::{Id, Vec3};
use crate::components::hud::Hud;
use bevy::asset::{AssetServer, Assets};
use bevy::input::ButtonInput;
use bevy::prelude::{error, info, warn, AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationPlayer, Camera, Capsule3d, ChildOf, Command, Component, Entity, EventReader, Gizmos, GlobalTransform, Handle, Local, Node, Quat, Reflect, Resource, Scene, SceneRoot, Time, Val, Vec2, World};
use bevy::prelude::{
    Camera3d, Commands, KeyCode, Mesh3d, MeshMaterial3d, Query, ReflectResource, Res, ResMut, Text, TextLayout, Transform, With,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;
use std::time::Duration;
use avian3d::prelude::{Collider, CollisionLayers, Friction, LayerMask, LinearVelocity, LockedAxes, Physics, PhysicsSchedule, Position, RigidBody, Rotation, Sleeping};
use bevy::color::palettes::basic::{PURPLE, WHITE};
use bevy::gltf::GltfAssetLabel;
use bevy::input::mouse::{AccumulatedMouseMotion, MouseMotion};
use bevy::math::EulerRot::YXZ;
use bevy::text::{FontSmoothing, TextFont};
use bevy::ui::PositionType;
use bevy::utils::default;
use crate::components::camera::{apply_player_camera_input, CameraInfo};
use crate::components::CollisionLayer;
use crate::components::player::animation::{AnimationState, PlayerAnimationState};
use crate::DefaultFont;

#[derive(Reflect, Hash, PartialEq, Eq, Clone, Copy, Debug)]
pub enum MovementState {
    Idle,
    Walking,
    Running,
    Jumping,
    Falling,
}

#[derive(Component)]
pub struct Controllable;

#[derive(Reflect, Resource, Default)]
#[reflect(Resource)]
pub struct PlayerInfo {
    pub current_player_id: Id,
    pub player_inputs: u16,
    pub mouse_delta: Vec2,
    pub accumulated_mouse_delta: Vec2,
    pub player_movement_state: HashSet<MovementState>,
}

#[derive(Component)]
pub struct PlayerMarker;

#[derive(Serialize, Deserialize, Debug, Default, Copy, Clone, PartialEq)]
pub struct Player {
    pub position: Vec3,
    pub linear_velocity: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub animation_state: AnimationState
}

impl Player {
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

pub fn set_player_id(
    player_info: &mut ResMut<PlayerInfo>,
    player_id: Id,
) {
    player_info.current_player_id = player_id;
}

const WALK_SPEED: f32 = 1.5;
const RUN_SPEED: f32 = 5.0;

fn apply_player_movement_input(
    encoded_input: u16,
    linear_velocity: &mut LinearVelocity,
    rotation: &mut Rotation,
    yaw: &f32,
) {
    let mut vector = bevy::math::Vec3::ZERO;

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
        linear_velocity.0.y += 1.0;
    }

    let normalized_rotated_velocity = Quat::from_euler(YXZ, *yaw, 0.0, 0.0).mul_vec3(vector.normalize_or_zero());

    // println!("normalized_velocity: {:?}", normalized_velocity);

    linear_velocity.0.x = normalized_rotated_velocity.x * WALK_SPEED;
    linear_velocity.0.z = normalized_rotated_velocity.z * WALK_SPEED;
    rotation.0 = Quat::from_euler(YXZ, *yaw, 0.0, 0.0).into();
}

pub fn player_controller(
    mut player_info: ResMut<PlayerInfo>,
    mut players: Query<(&Id, &Transform, &mut LinearVelocity, &mut Rotation, &mut CameraInfo, &mut PlayerAnimationState), With<PlayerMarker>>,
    mut hud: Query<&mut Text, With<Hud>>,
) {
        for (id, transform, mut linear_velo, mut rotation, mut camera_info, mut player_anim_state) in players.iter_mut() {
            if player_info.current_player_id == *id {
                if player_info.player_inputs != 0 {
                    player_anim_state.0 = AnimationState::Walking;
                    apply_player_movement_input(player_info.player_inputs, &mut linear_velo, &mut rotation, &camera_info.yaw);
                } else {
                    player_anim_state.0 = AnimationState::Idle;
                }

                if let Some(mut h) = hud.single_mut().ok() {
                    h.0.clear();
                    h.0.push_str(&format!(
                        "x: {:?}\ny: {:?}\nz: {:?}\n",
                        transform.translation.x, transform.translation.y, transform.translation.z
                    ));
                }

                let position = Vec3::new(
                    transform.translation.x,
                    transform.translation.y,
                    transform.translation.z,
                );
            }
        }
        player_info.accumulated_mouse_delta = Vec2::ZERO;
}

pub fn update_players(
    commands: &mut Commands,
    default_font: &Res<DefaultFont>,
    asset_server: &Res<AssetServer>,
    animation_graphs: &mut Assets<AnimationGraph>,
    server_players: &HashMap<Id, Player>,
    client_players: &mut Query<(&mut Transform, &Id, Entity, &CameraInfo, &mut PlayerAnimationState), With<PlayerMarker>>,
    info: &Res<PlayerInfo>,
) {
    let mut existing_players = HashSet::new();

    for (mut transform, id, entity, _, mut anim_state) in client_players.iter_mut() {
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

            transform.translation.x = player.position.x;
            transform.translation.y = player.position.y;
            transform.translation.z = player.position.z;
            transform.rotation = Quat::from_euler(YXZ, player.yaw, 0.0, 0.0);
            anim_state.0 = player.animation_state;
        }
    }

    // Spawns players if they do not exist
    for p in server_players.iter() {
        if !existing_players.contains(p.0) {
            println!("{:?}", p.1.position);

            let player = commands.spawn((
                RigidBody::Dynamic,
                Collider::capsule(0.5, 1.0),
                Friction::new(1.0),
                LockedAxes::new().lock_rotation_x().lock_rotation_y().lock_rotation_z(),
                Position::from_xyz(p.1.position.x, p.1.position.y, p.1.position.z),
                CollisionLayers::new(CollisionLayer::Player, [LayerMask::ALL]),
                Transform::default().with_scale(bevy::math::Vec3::splat(1.0)),
                CameraInfo{ yaw: p.1.yaw, pitch: p.1.pitch },
                PlayerAnimationState(AnimationState::Idle),
                *p.0,
                PlayerMarker
            )).with_children( |parent| {
                parent.spawn((
                    SceneRoot(asset_server.load(GltfAssetLabel::Scene(0).from_asset("meshes\\player.glb"))),
                    Transform::from_xyz(0.0, -1.0, 0.0).with_rotation(Quat::from_euler(YXZ, PI, 0.0, 0.0)),
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
                        line_height: Default::default(),
                        font_smoothing: FontSmoothing::None,
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
            let pos = world_position.translation() + bevy::math::Vec3::Y;
            
            let (camera, camera_transform) = camera3d.single().unwrap();

            let viewport_position = match camera.world_to_viewport(camera_transform, pos) {
                Ok(v) => v,
                Err(e) => { /*println!("{:?}", e);*/ continue; },
            };

            node.top = Val::Px(viewport_position.y);
            node.left = Val::Px(viewport_position.x);
        } else {
            commands.entity(entity).despawn();
        }
    }
}