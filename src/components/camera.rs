use avian3d::prelude::Position;
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::{Camera3d, Component, KeyCode, Local, MessageReader, Quat, Query, Res, Single, Time, Transform, Vec3, Window, With, Without};
use bevy::prelude::EulerRot::YXZ;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use crate::components::common::{Id, Vec2};
use crate::components::player::{PlayerInfo, PlayerMarker, PredictedPlayerState};

const LOOK_SENSITIVITY: (f32, f32) = (0.001, 0.001);
const CAM_SPACE: f32 = 10.0;
const CAMERA_HEIGHT: f32 = 0.75;
const CAMERA_FORWARD: f32 = 0.5;

#[derive(Component, Default, Debug)]
pub struct CameraInfo {
    pub yaw: f32,
    pub pitch: f32,
}

pub fn apply_player_camera_input (
    mouse_delta: Vec2,
    predicted_player_state: &mut PredictedPlayerState,
) {
    predicted_player_state.predicted_yaw += -1.0 * LOOK_SENSITIVITY.0 * mouse_delta.x;
    predicted_player_state.predicted_pitch += 1.0 * LOOK_SENSITIVITY.1 * mouse_delta.y;

    predicted_player_state.predicted_pitch = predicted_player_state.predicted_pitch.clamp(-90.0f32.to_radians(), 90.0f32.to_radians());
}

pub(crate) fn camera_controller(
    mut camera: Query<&mut Transform, (With<Camera3d>, Without<PlayerMarker>)>,
    mut player: Query<(&Id, &Position, &mut PredictedPlayerState), (With<PlayerMarker>, Without<Camera3d>)>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    player_info: Res<PlayerInfo>,
    mut zoom: Local<f32>
) {
    for ev in mouse_wheel.read() {
        *zoom -= ev.y;
        *zoom = zoom.clamp(-0.2, 10.0);
    }
    
    for (id, position, mut predicted_state) in player.iter_mut() {
        if *id == player_info.current_player_id {
            apply_player_camera_input(player_info.mouse_delta.into(), &mut predicted_state);

            for mut cam in camera.iter_mut() {
                cam.rotation = Quat::from_euler(YXZ, predicted_state.predicted_yaw, -predicted_state.predicted_pitch, 0.0);

                let pivot_shift = position.0 + Vec3::new(0.0, CAMERA_HEIGHT, 0.0);
                
                if CAM_SPACE == 0. {
                    cam.translation = pivot_shift + Vec3::new(0.0, 0.0, *zoom); // 0.0, 0.5, 2.0
                } else {
                    cam.translation = pivot_shift + cam.rotation * Vec3::new(0.0, 0.0, *zoom); // 0.0, 0.5, 2.0
                }
            }
        }
    }
}

pub fn lock_cursor_system(
    mut cursor_options: Single<&mut CursorOptions>,
    mut keyboard_input: MessageReader<KeyboardInput>,
    mut toggle_cursor_lock: Local<bool>,
) {
   for ev in keyboard_input.read() {
       if ev.state == ButtonState::Pressed && ev.key_code == KeyCode::Tab {
           if *toggle_cursor_lock {
               cursor_options.grab_mode = CursorGrabMode::Locked;
               cursor_options.visible = false;
           } else {
               cursor_options.grab_mode = CursorGrabMode::None;
               cursor_options.visible = true;
           }

           *toggle_cursor_lock = !*toggle_cursor_lock;
       }
   } 
}