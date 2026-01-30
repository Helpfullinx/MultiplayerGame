use std::collections::VecDeque;
use std::f32::consts::PI;
use avian3d::debug_render::PhysicsDebugPlugin;
use avian3d::PhysicsPlugins;
use avian3d::prelude::{Collider, CollisionLayers, LayerMask, Physics, RigidBody};
use bevy::app::{App, FixedUpdate, Startup, Update};
use bevy::asset::{AssetServer, Assets, Handle};
use bevy::color::Color;
use bevy::core_pipeline::Skybox;
use bevy::DefaultPlugins;
use bevy::dev_tools::fps_overlay::FpsOverlayPlugin;
use bevy::image::Image;
use bevy::math::{Quat, Vec3};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::{default, Camera3d, Commands, Cuboid, DirectionalLight, Fixed, Font, Mesh, Mesh3d, Msaa, Node, Plugin, PositionType, Query, Res, ResMut, Resource, Text, TextFont, Time, Transform, Val};
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
use bevy::text::FontSmoothing;
use bevy_inspector_egui::bevy_egui::EguiPlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use crate::components::player::plugin::PlayerPlugin;
use crate::components::chat::{chat_window, Chat};
use crate::components::CollisionLayer;
use crate::components::hud::Hud;
use crate::components::lobby::join_lobby;
use crate::components::weapon::Weapon;
use crate::network::net_plugin::{NetworkConfig, NetworkPlugin, RemoteAddress};
use crate::network::net_plugin::HostType::Client;

#[derive(Resource)]
pub struct DefaultFont(pub Handle<Font>);

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            DefaultPlugins,
            PhysicsPlugins::default().with_length_unit(10.0),
            EguiPlugin::default(),
            WorldInspectorPlugin::new(),
            FpsOverlayPlugin::default(),
            PhysicsDebugPlugin::default(),
            NetworkPlugin::new(NetworkConfig{ host_type: Client }),
            PlayerPlugin
        ));
        app.insert_resource(Time::<Fixed>::from_hz(60.0));
        app.insert_resource(Time::<Physics>::default());
        app.insert_resource(DefaultFont(Handle::default()));
        app.insert_resource(RemoteAddress("127.0.0.1:4444".to_string()/*remote_address.clone()*/));
        app.add_systems(Startup, setup);
        app.add_systems(Update, asset_loaded);
        app.add_systems(
            FixedUpdate,
            (
                join_lobby,
                chat_window
            )
        );
    }
}

fn setup(
    mut default_font: ResMut<DefaultFont>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    default_font.0 = asset_server.load("fonts\\alagard.ttf");
    let skybox_handle = asset_server.load("skybox\\sky1.png");

    println!("{:?}", default_font.0);

    // Main Camera
    commands.spawn((
        Camera3d::default(),
        Msaa::Sample8,
        // TemporalAntiAliasing::default(),
        // ScreenSpaceAmbientOcclusion { quality_level: ScreenSpaceAmbientOcclusionQualityLevel::Ultra, constant_object_thickness: 4.0 },
        // TemporalJitter::default(),
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Skybox {
            image: skybox_handle.clone(),
            brightness: 1000.0,
            ..default()
        },
    ));

    commands.insert_resource(Cubemap {
        is_loaded: false,
        image_handle: skybox_handle,
    });

    // Ground Plane
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(40.0, 0.5, 40.0),
        CollisionLayers::new(CollisionLayer::Ground, [LayerMask::ALL]),
        Mesh3d(meshes.add(Cuboid::new(40.0,0.5,40.0))),
        MeshMaterial3d(materials.add(Color::WHITE)),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    //Light Source
    commands.spawn((
        DirectionalLight {
            illuminance: 32000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 2.0, 0.0).with_rotation(Quat::from_rotation_x(-PI / 4.)),
    ));

    //Position and ID Hud
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont {
            font: default_font.0.clone(),
            font_size: 20.0,
            font_smoothing: FontSmoothing::None,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.5),
            right: Val::Px(0.5),
            ..default()
        },
    ));

    // Chat Window
    commands.spawn((
        Chat {
            chat_history: VecDeque::new(),
        },
        Text::new(""),
        TextFont {
            font: default_font.0.clone(),
            font_size: 20.0,
            font_smoothing: FontSmoothing::None,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.5),
            left: Val::Px(0.5),
            ..default()
        },
    ));

    commands.spawn(Weapon{ damage: 10, range: 100.0 });
}

#[derive(Resource)]
struct Cubemap {
    is_loaded: bool,
    image_handle: Handle<Image>,
}

fn asset_loaded(
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut cubemap: ResMut<Cubemap>,
    mut skyboxes: Query<&mut Skybox>,
) {
    if !cubemap.is_loaded && asset_server.load_state(&cubemap.image_handle).is_loaded() {
        let image = images.get_mut(&cubemap.image_handle).unwrap();
        // NOTE: PNGs do not have any metadata that could indicate they contain a cubemap texture,
        // so they appear as one texture. The following code reconfigures the texture as necessary.
        if image.texture_descriptor.array_layer_count() == 1 {
            _ = image.reinterpret_stacked_2d_as_array(image.height() / image.width());
            image.texture_view_descriptor = Some(TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                ..default()
            });
        }

        for mut skybox in &mut skyboxes {
            skybox.image = cubemap.image_handle.clone();
        }

        cubemap.is_loaded = true;
    }
}