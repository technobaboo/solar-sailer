use std::collections::HashMap;

use glam::Vec3;
use gluon_ipc::{Interface, Node, RefExt};
use stardust_xr_fusion::{
	client::{Client, ClientHandler},
	fields::{Field, FieldExt, FieldRef, FieldSample, Shape},
	query::{InterfaceDependency, QueriedInterface, QueryableId},
	spatial::{Spatial, SpatialExt, SpatialRef, Transform},
	spatial_query::{ZoneQuery, ZoneQueryHandle, ZoneQueryHandler, ZoneQueryHandlerHandler},
	types::Vec3F,
};
use stardust_xr_molecules::{environment::Environment, transformable::protocol::Translatable};
use tokio::sync::RwLock;

pub struct TranslateMovement {
	translatables: Node<Translatables>,
	_field: Field,
	_spatial: Spatial,
	_query: ZoneQueryHandle,
}
#[derive(gluon_ipc::Handler)]
struct Translatables(RwLock<HashMap<QueryableId, Translatable>>);
impl Translatables {
	fn find(interfaces: Vec<QueriedInterface>) -> Option<Translatable> {
		interfaces
			.into_iter()
			.find(|i| i.interface_id == Translatable::ID)
			.map(|i| Translatable::from_ref(i.interface))
	}

	async fn update(&self, id: QueryableId, interfaces: Vec<QueriedInterface>) {
		let mut translatables = self.0.write().await;
		match Self::find(interfaces) {
			Some(translatable) => translatables.insert(id, translatable),
			None => translatables.remove(&id),
		};
	}
}
impl ZoneQueryHandlerHandler for Translatables {
	async fn entered(
		&self,
		_ctx: gluon_ipc::Context,
		id: QueryableId,
		_field: FieldRef,
		_spatial: SpatialRef,
		interfaces: Vec<QueriedInterface>,
		_relative_position: Vec3F,
		_sample: FieldSample,
	) {
		self.update(id, interfaces).await;
	}

	async fn interfaces_changed(
		&self,
		_ctx: gluon_ipc::Context,
		id: QueryableId,
		interfaces: Vec<QueriedInterface>,
	) {
		self.update(id, interfaces).await;
	}

	async fn moved(
		&self,
		_ctx: gluon_ipc::Context,
		_id: QueryableId,
		_relative_position: Vec3F,
		_sample: FieldSample,
	) {
	}

	async fn left(&self, _ctx: gluon_ipc::Context, id: QueryableId) {
		self.0.write().await.remove(&id);
	}
}

impl TranslateMovement {
	pub async fn new(client: &Client<impl ClientHandler>) -> stardust_xr_fusion::Result<Self> {
		let (spatial, _) = Spatial::new(client, client.root(), Transform::IDENTITY).await?;
		let (field, field_ref) =
			Field::new(client, &spatial, Shape::Sphere { radius: f32::MAX }).await?;
		let (translatables, translatables_ref) =
			ZoneQueryHandler::new_node(Translatables(RwLock::default()))?;
		let query = client
			.spatial_query_interface()
			.zone_query(ZoneQuery {
				handler: translatables_ref.into_proxy(),
				interfaces: vec![
					InterfaceDependency {
						id: Translatable::ID.to_string(),
						optional: false,
					},
					InterfaceDependency {
						id: Environment::ID.to_string(),
						optional: false,
					},
				],
				zone_field: field_ref,
				margin: 0.0,
			})
			.await??;

		Ok(TranslateMovement {
			translatables,
			_field: field,
			_spatial: spatial,
			_query: query,
		})
	}

	pub async fn apply_offset(
		&mut self,
		delta_secs: f32,
		velocity_ref: &SpatialRef,
		velocity: Vec3,
	) {
		let offset: Vec3F = (velocity * delta_secs).into();
		for translatable in self.translatables.0.read().await.values() {
			let _ = translatable.offset_relative_translation(velocity_ref.clone(), offset);
		}
	}
}
