use bevy::app::{App, Plugin};
use avian3d::prelude::*;
use lightyear::prelude::{AppComponentExt, InterpolationRegistrationExt, PredictionRegistrationExt};
use crate::components::player;
use crate::components::player::MovementState;

#[derive(Clone)]
struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        //register all types that should be shared over the network here
        
        app.register_component::<Position>()
            .add_prediction()
            .add_should_rollback(position_should_rollback)
            .add_linear_correction_fn()
            .add_linear_interpolation();

        app.register_component::<Rotation>()
            .add_prediction()
            .add_should_rollback(rotation_should_rollback)
            .add_linear_correction_fn()
            .add_linear_interpolation();
    }
}

fn position_should_rollback(this: &Position, that: &Position) -> bool {
    (this.0 - that.0).length() >= 0.1
}

fn rotation_should_rollback(this: &Rotation, that: &Rotation) -> bool {
    this.0.angle_between(that.0) >= 0.1
}