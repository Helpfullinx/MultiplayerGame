use std::collections::HashSet;
use bevy::app::{App, FixedPostUpdate, FixedPreUpdate, Plugin, PostUpdate};
use bevy::math::Vec2;
use bevy::prelude::{FixedUpdate, IntoScheduleConfigs, PreUpdate, Update};
use crate::components::camera::{camera_controller, lock_cursor_system};
use crate::components::common::Id;
use crate::components::player::{player_controller, update_label_pos, update_player_kinematics, PlayerInfo};
use crate::components::player::animation::{animation_control, player_animations, setup_player_animations};
use crate::components::player::input::input_system;
use crate::components::weapon::weapon_controller;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PlayerInfo {
            current_player_id: Id(0),
            player_inputs: 0,
            mouse_delta: Vec2::ZERO.into(),
            accumulated_mouse_delta: Vec2::ZERO.into(),
            player_movement_state: HashSet::new()
        });
        app.add_systems(PreUpdate, (
            input_system,
        ));
        app.add_systems(
            Update, 
            (
                lock_cursor_system,
                camera_controller,
                update_label_pos,
                setup_player_animations,
                weapon_controller,
            )
        );
        // app.add_systems(
        //     FixedPreUpdate, (
        //         
        //     )
        // );
        app.add_systems(
            FixedUpdate,
            (
                player_controller,
                update_player_kinematics,
                player_animations,
                animation_control
            ).chain()
        );
    }
}