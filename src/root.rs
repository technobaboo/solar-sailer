use stardust_xr_fusion::{
	client::Client,
	drawable::Model,
	fields::{Field, Shape},
	node::{MethodResult, NodeResult},
	root::{ClientState, FrameInfo, RootHandler},
	spatial::{SpatialAspect, SpatialRefAspect, Transform},
	values::ResourceID,
};
use stardust_xr_molecules::{Grabbable, GrabbableSettings, PointerMode};
use std::sync::Arc;

pub struct ClientRoot {
	grabbable: Grabbable,
	_field: Field,
	_model: Model,
}
impl ClientRoot {
	pub async fn new(client: &Arc<Client>) -> NodeResult<Self> {
		let model = Model::create(
			client.get_root(),
			Transform::from_scale([0.25; 3]),
			&ResourceID::new_namespaced("template", "color_cube"),
		)?;
		let model_bounds = model.get_local_bounding_box().await?;
		let field = Field::create(&model, Transform::identity(), Shape::Box(model_bounds.size))?;
		let grabbable = Grabbable::create(
			client.get_root(),
			Transform::identity(),
			&field,
			GrabbableSettings {
				max_distance: 0.05,
				magnet: false,
				pointer_mode: PointerMode::Parent,
				zoneable: true,
				..Default::default()
			},
		)?;
		model.set_spatial_parent_in_place(grabbable.content_parent())?;

		Ok(Self {
			grabbable,
			_field: field,
			_model: model,
		})
	}
}
impl RootHandler for ClientRoot {
	fn frame(&mut self, info: FrameInfo) {
		let _ = self.grabbable.update(&info);
	}
	fn save_state(&mut self) -> MethodResult<ClientState> {
		Ok(ClientState::default())
	}
}
