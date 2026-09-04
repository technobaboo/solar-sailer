use glam::{Affine3A, Vec3};
use libmonado::Monado;
use stardust_xr_fusion::{
	client::{Client, ClientHandler},
	spatial::Transform,
};
use tracing::error;

use crate::{input::Input, monado_movement::MonadoMovement, translate_movement::TranslateMovement};

pub struct SolarSailer {
	monado_movement: Option<MonadoMovement>,
	mode: Mode,
	input: Input,
	translate_movement: TranslateMovement,
	velocity: Vec3,
}

impl SolarSailer {
	pub async fn new(client: &Client<impl ClientHandler>, input: Input) -> Self {
		let monado = match Monado::auto_connect() {
			Ok(v) => Some(v),
			Err(err) => {
				error!("Couldn't connect to monado :( {err}");
				None
			}
		};
		let monado_movement = MonadoMovement::from_monado(monado).await;

		SolarSailer {
			mode: match monado_movement.is_some() {
				true => Mode::MonadoOffset,
				false => Mode::Translate,
			},
			monado_movement,
			input,
			translate_movement: TranslateMovement::new(client).await.unwrap(),
			velocity: Vec3::ZERO,
		}
	}
	pub fn should_switch_mode(&mut self) -> bool {
		self.input.update_mode()
	}
	pub fn handle_input(&mut self) -> impl Future {
		self.input.handle_input()
	}
	pub async fn apply_offset(&mut self, client: &Client<impl ClientHandler>, delta_secs: f32) {
		let vel_ref = &self.input.get_velocity_space();
		if self.velocity.length_squared() <= 0.0005 {
			return;
		}
		match (&self.mode, self.monado_movement.as_mut()) {
			(Mode::MonadoOffset, Some(monado)) => {
				monado
					.apply_offset(client, delta_secs, vel_ref, self.velocity)
					.await
			}
			(Mode::Translate, _) => {
				self.translate_movement
					.apply_offset(delta_secs, vel_ref, self.velocity)
					.await
			}
			_ => {}
		}
	}

	pub fn current_mode(&self) -> Mode {
		self.mode
	}

	pub fn switch_mode(&mut self, mode: Mode) {
		self.mode = mode;
	}

	pub async fn update_velocity(&mut self, delta_secs: f32) {
		let offset = self.input.waft(delta_secs).await;
		self.velocity *= 0.99;
		self.velocity += offset;
	}
	pub fn update_signifiers(&self) {
		self.input.update_signifiers(self.mode);
	}
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum Mode {
	Translate,
	MonadoOffset,
	Disabled,
}

pub fn mat_from_transform(transform: &Transform) -> Affine3A {
	Affine3A::from_scale_rotation_translation(
		transform.scale.into(),
		transform.rotation.into(),
		transform.translation.into(),
	)
}
