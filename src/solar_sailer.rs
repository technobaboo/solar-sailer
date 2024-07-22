use glam::{Affine3A, Quat, Vec3};
use libmonado_rs::{Monado, Pose};
use send_wrapper::SendWrapper;
use stardust_xr_fusion::{
	client::Client,
	drawable::{Line, LinePoint, Lines},
	fields::{CylinderShape, Field, Shape},
	input::{InputDataType, InputHandler},
	node::{MethodResult, NodeResult},
	root::{ClientState, FrameInfo, RootHandler},
	spatial::{Spatial, SpatialAspect, Transform},
	values::color::rgba_linear,
};
use stardust_xr_molecules::input_action::{InputQueue, InputQueueable, SimpleAction, SingleAction};

pub struct SolarSailer {
	monado: SendWrapper<Monado>,
	velocity: Vec3,
	previous_position: Vec3,

	pen_root: Spatial,
	_field: Field,
	input: InputQueue,
	grab_action: SingleAction,
	glide_action: SimpleAction,
	_visuals: Lines,
}
impl SolarSailer {
	pub fn new(monado: Monado, stardust_client: &Client, thickness: f32) -> NodeResult<Self> {
		let visual_length = 0.075;
		let pen_root = Spatial::create(stardust_client.get_root(), Transform::none(), true)?;
		let field = Field::create(
			&pen_root,
			Transform::from_translation([0.0, 0.0, visual_length * 0.5]),
			Shape::Cylinder(CylinderShape {
				length: visual_length,
				radius: thickness * 0.5,
			}),
		)?;
		let input =
			InputHandler::create(stardust_client.get_root(), Transform::none(), &field)?.queue()?;

		let color = rgba_linear!(1.0, 1.0, 0.0, 1.0);
		let visuals = Lines::create(
			&pen_root,
			Transform::none(),
			&[Line {
				points: vec![
					LinePoint {
						point: [0.0; 3].into(),
						thickness: 0.0,
						color,
					},
					LinePoint {
						point: [0.0, 0.0, thickness].into(),
						thickness,
						color,
					},
					LinePoint {
						point: [0.0, 0.0, visual_length].into(),
						thickness,
						color,
					},
				],
				cyclic: false,
			}],
		)?;

		for origin in monado.tracking_origins().unwrap().into_iter() {
			let _ = origin.set_offset(Pose {
				position: Vec3::ZERO.into(),
				orientation: Quat::IDENTITY.into(),
			});
		}
		Ok(Self {
			monado: SendWrapper::new(monado),
			velocity: Default::default(),
			previous_position: Default::default(),

			pen_root,
			_field: field,
			input,
			grab_action: Default::default(),
			glide_action: Default::default(),
			_visuals: visuals,
		})
	}
}
impl RootHandler for SolarSailer {
	fn frame(&mut self, info: FrameInfo) {
		self.velocity *= 0.99;

		let origins = self
			.monado
			.tracking_origins()
			.unwrap()
			.into_iter()
			.collect::<Vec<_>>();

		let Some(Pose {
			position,
			orientation,
		}) = origins.first().and_then(|o| o.get_offset().ok())
		else {
			return;
		};
		let real_to_offset_matrix =
			Affine3A::from_rotation_translation(orientation.into(), position.into());
		// dbg!(self.velocity.length() * info.delta);
		if self.velocity.length_squared() > 0.001 {
			// dbg!(monado_offset_position);
			let delta_position = self.velocity * info.delta;
			let offset_position =
				real_to_offset_matrix.transform_vector3(delta_position) + Vec3::from(position);
			// offset_position.y = offset_position.y.max(0.0);
			// dbg!(offset_position);

			for origin in origins.iter() {
				let _ = origin.set_offset(Pose {
					position: offset_position.into(),
					orientation,
				});
			}
		}

		// for origin in origins.iter() {
		// 	let _ = origin.set_offset(Pose {
		// 		position: [0.0, info.elapsed.sin(), 0.0].into(),
		// 		orientation,
		// 	});
		// }

		self.grab_action.update(
			false,
			&self.input,
			|data| data.distance < 0.05,
			|data| {
				data.datamap.with_data(|datamap| match &data.input {
					InputDataType::Hand(_) => datamap.idx("grab_strength").as_f32() > 0.90,
					InputDataType::Tip(_) => datamap.idx("grab").as_f32() > 0.90,
					_ => false,
				})
			},
		);
		self.glide_action.update(&self.input, &|data| {
			data.datamap.with_data(|datamap| match &data.input {
				InputDataType::Hand(h) => {
					Vec3::from(h.thumb.tip.position).distance(h.index.tip.position.into()) < 0.03
				}
				InputDataType::Tip(_) => datamap.idx("select").as_f32() > 0.01,
				_ => false,
			})
		});

		if self.grab_action.actor_started() {
			let _ = self.pen_root.set_zoneable(false);
		}
		if self.grab_action.actor_stopped() {
			let _ = self.pen_root.set_zoneable(true);
		}
		let Some(grab_actor) = self.grab_action.actor() else {
			return;
		};
		let transform = match &grab_actor.input {
			InputDataType::Hand(h) => Transform::from_translation_rotation(
				(Vec3::from(h.thumb.tip.position) + Vec3::from(h.index.tip.position)) * 0.5,
				h.palm.rotation,
			),
			InputDataType::Tip(t) => Transform::from_translation_rotation(t.origin, t.orientation),
			_ => Transform::none(),
		};
		let _ = self
			.pen_root
			.set_relative_transform(self.input.handler(), transform);

		let position = Vec3::from(match &grab_actor.input {
			InputDataType::Hand(h) => h.palm.position,
			InputDataType::Tip(t) => t.origin,
			_ => unreachable!(),
		});
		let real_position = real_to_offset_matrix.inverse().transform_point3(position);
		// let real_position = position;
		// dbg!(real_position);

		if self.glide_action.started_acting().contains(grab_actor) {
			self.previous_position = real_position;
		}
		if self.glide_action.currently_acting().contains(grab_actor) {
			let offset = self.previous_position - real_position;
			// dbg!(offset);
			let offset_magnify = (offset.length()).powf(0.9);
			// dbg!(offset_magnify);
			self.velocity += offset.normalize_or_zero() * offset_magnify;
		}
		self.previous_position = real_position;
	}

	fn save_state(&mut self) -> MethodResult<ClientState> {
		ClientState::from_root(&self.pen_root)
	}
}
