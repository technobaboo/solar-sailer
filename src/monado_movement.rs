use glam::Vec3;
use libmonado::{Monado, Pose};
use stardust_xr_fusion::{
	client::{Client, ClientHandler},
	spatial::SpatialRef,
	tracked::{Tracked, TrackedExt},
};
use tracing::error;

use crate::solar_sailer::mat_from_transform;

pub struct MonadoMovement {
	monado: Monado,
	stage: SpatialRef,
}

impl MonadoMovement {
	pub async fn apply_offset(
		&mut self,
		client: &Client<impl ClientHandler>,
		delta_secs: f32,
		velocity_ref: &SpatialRef,
		velocity: Vec3,
	) {
		let Ok(origins) = self
			.monado
			.tracking_origins()
			.inspect_err(|err| error!("unable to get monado origins: {err}"))
		else {
			return;
		};

		let Ok(Ok(transform)) = client
			.spatial_interface()
			.get_relative_transform(self.stage.clone(), velocity_ref.clone())
			.await
			.map(|v| {
				v.inspect_err(|err| {
					error!("unable to get velocity_ref to stage transform: {err:?}")
				})
			})
			.inspect_err(|err| error!("unable to get velocity_ref to stage transform: {err}"))
		else {
			return;
		};
		let mat = mat_from_transform(&transform);
		let delta_position = mat.transform_vector3(-velocity * delta_secs);

		for origin in origins {
			let Some(Pose {
				position,
				orientation,
			}) = origin.get_offset().ok()
			else {
				continue;
			};
			let offset_position = Vec3::from(position) + (delta_position);

			let _ = origin.set_offset(Pose {
				position: offset_position.into(),
				orientation,
			});
		}
	}

	pub async fn from_monado(monado: Option<Monado>) -> Option<Self> {
		let monado = monado?;
		Some(MonadoMovement {
			monado,
			stage: Tracked::stage_spatial().await.ok()?,
		})
	}
}
