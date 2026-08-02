use std::{future::ready, sync::OnceLock};

use glam::{Affine3A, Vec3};
use gluon::{Handler, Object};
use stardust_xr_fusion::{
	client::{Client, ClientHandler},
	fields::{Field, FieldExt, FieldRef, FieldSample, Shape},
	query::{InterfaceDependency, QueriedInterface, QueryableObjectRef},
	spatial::{Spatial, SpatialExt, SpatialRef, Transform},
	spatial_query::{SpatialQueryGuard, ZoneQuery, ZoneQueryHandler, ZoneQueryHandlerHandler},
	types::Vec3F,
};
use stardust_xr_molecules::reparentable::{
	self, REPARENTABLE_PROTOCOL, ReparentHandle, ReparentKeepaliveHandler, ReparentableProxy,
};
use tokio::sync::RwLock;
use tracing::error;

use crate::solar_sailer::mat_from_transform;

pub struct ReparentMovement {
	spatial: Spatial,
	spatial_ref: SpatialRef,
	reparent: Option<Object<Reparent>>,
}
#[derive(Debug, Handler)]
struct Reparent {
	spatial_ref: SpatialRef,
	reparented: RwLock<Vec<ReparentHandle>>,
	keepalive: Object<ReparentKeepalive>,
	guard: OnceLock<SpatialQueryGuard>,
}
#[derive(Debug, Handler)]
struct ReparentKeepalive;
impl ReparentKeepaliveHandler for ReparentKeepalive {
	fn reparent_stolen(&self, _ctx: gluon::Context) -> impl Future<Output = ()> + Send + Sync {
		ready(())
	}
}
impl ZoneQueryHandlerHandler for Reparent {
	async fn entered(
		&self,
		_ctx: gluon::Context,
		_obj: QueryableObjectRef,
		_field: FieldRef,
		_spatial: SpatialRef,
		interfaces: Vec<QueriedInterface>,
		_relative_position: Vec3F,
		_sample: FieldSample,
	) {
		let reparentable = ReparentableProxy::from_object_or_ref(interfaces[0].interface.clone());
		let Ok(Some(handle)) = reparentable
			.reparent(
				self.spatial_ref.clone(),
				reparentable::ReparentKeepalive::from_handler(&self.keepalive),
			)
			.await
		else {
			return;
		};
		self.reparented.write().await.push(handle);
	}

	fn interfaces_changed(
		&self,
		_ctx: gluon::Context,
		_obj: QueryableObjectRef,
		_interfaces: Vec<QueriedInterface>,
	) -> impl Future<Output = ()> + Send + Sync {
		ready(())
	}

	fn moved(
		&self,
		_ctx: gluon::Context,
		_obj: QueryableObjectRef,
		_relative_position: Vec3F,
		_sample: FieldSample,
	) -> impl Future<Output = ()> + Send + Sync {
		ready(())
	}

	fn left(
		&self,
		_ctx: gluon::Context,
		_obj: QueryableObjectRef,
	) -> impl Future<Output = ()> + Send + Sync {
		ready(())
	}
}
impl Reparent {
	async fn new(
		client: &Client<impl ClientHandler>,
		movement: &ReparentMovement,
	) -> Option<Object<Self>> {
		let keepalive = client.pion_device().register_object(ReparentKeepalive);
		let reparent = client.pion_device().register_object(Reparent {
			spatial_ref: movement.spatial_ref.clone(),
			reparented: RwLock::default(),
			keepalive,
			guard: OnceLock::new(),
		});
		let (_, field_ref) = Field::new(
			client,
			&movement.spatial,
			Shape::Sphere { radius: f32::MAX },
		)
		.await
		.ok()?;
		let query = client
			.spatial_query_interface()
			.zone_query(ZoneQuery {
				handler: ZoneQueryHandler::from_handler(&reparent),
				interfaces: vec![InterfaceDependency {
					id: REPARENTABLE_PROTOCOL.protocol_name.into(),
					optional: false,
				}],
				zone_field: field_ref,
				margin: 0.0,
			})
			.await
			.ok()?
			.ok()?;
		_ = reparent.guard.set(query);

		Some(reparent)
	}
}

impl ReparentMovement {
	pub async fn apply_offset(
		&mut self,
		client: &Client<impl ClientHandler>,
		delta_secs: f32,
		velocity_ref: &SpatialRef,
		velocity: Vec3,
	) {
		if self.reparent.is_none() {
			self.reparent = Reparent::new(client, self).await;
		}

		let transform = client
			.spatial_interface()
			.get_relative_transform(self.spatial_ref.clone(), velocity_ref.clone())
			.await
			.unwrap()
			.unwrap();
		let mat = mat_from_transform(&transform);
		let movement = mat.transform_vector3(velocity * delta_secs);
		let offset = Affine3A::from_translation(movement);
		if let Err(err) = self.spatial.set_relative_transform(
			velocity_ref.clone(),
			Transform::from_translation((offset * mat.inverse()).to_scale_rotation_translation().2),
		) {
			error!("unable to set transform: {err}");
		}
	}

	pub fn stopped_moving(&mut self) {
		self.reparent.take();
	}

	pub async fn new(client: &Client<impl ClientHandler>) -> stardust_xr_fusion::Result<Self> {
		let (spatial, spatial_ref) =
			Spatial::new(client, client.root(), Transform::IDENTITY).await?;
		Ok(ReparentMovement {
			spatial,
			spatial_ref,
			reparent: None,
		})
	}
}
