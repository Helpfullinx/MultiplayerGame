use std::collections::VecDeque;
use avian3d::PhysicsPlugins;
use avian3d::prelude::{Collider, CollisionLayers, LayerMask, Physics, PhysicsTime, RigidBody};
use bevy::app::App;
use bevy::log::LogPlugin;
use bevy::MinimalPlugins;
use bevy::prelude::{AssetPlugin, Assets, Commands, Fixed, Mesh, Plugin, Startup, Time, Transform, TransformPlugin, Update};
use bevy::scene::ScenePlugin;
use crate::components::chat::Chat;
use crate::components::CollisionLayer;
use crate::components::player::plugin::PlayerPlugin;
use crate::network::net_plugin::{HostType, NetworkConfig, NetworkPlugin};

pub struct ServerPlugin;

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin::default(),
            AssetPlugin::default(),
            ScenePlugin,
            LogPlugin::default(),
            PhysicsPlugins::default(),
            NetworkPlugin::new(NetworkConfig{ host_type: HostType::Server }),
            PlayerPlugin,
        ));
        app.init_resource::<Assets<Mesh>>();
        app.insert_resource(Time::<Fixed>::from_hz(60.0));
        app.insert_resource(Time::<Physics>::default());
        app.add_systems(Startup, setup);
    }
}

fn setup(mut commands: Commands) {
    commands.spawn(Chat {
        chat_history: VecDeque::new(),
    });

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.5, 40.0),
        Transform::from_xyz(0.0, 0.0, 0.0),
        CollisionLayers::new(CollisionLayer::Ground, [LayerMask::ALL]),
    ));
}