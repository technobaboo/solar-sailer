use std::{
	f32::consts::{FRAC_PI_2, FRAC_PI_4},
	process,
};

use glam::{Mat4, Quat, Vec3, vec3};
use stardust_xr_fusion::{
	client::{Client, ClientHandler},
	drawable::{Line, LinePoint, Lines, LinesExt, Model, ModelExt},
	fields::{Field, FieldExt, Shape},
	spatial::{PartialTransform, Spatial, SpatialExt, SpatialRef, Transform},
	suis::InputDataType,
	tracked::{Tracked, TrackedExt},
	types::{Resource, color::rgba, rgba_linear},
};
use stardust_xr_molecules::{
	Derezzable, UIElement,
	button::{Button, ButtonSettings},
	input_action::{InputQueue, InputSnapshot, SimpleAction, SingleAction},
	lines::{LineExt as _, circle},
	reparentable::Reparentable,
};
use tracing::error;

use crate::{
	APP_ID,
	mode_button::ModeButton,
	solar_sailer::{Mode, mat_from_transform},
};

pub struct PenInput {
	move_action: SimpleAction,
	grab_action: SingleAction,
	field: Field,
	field_spatial: Spatial,
	field_spatial_ref: SpatialRef,
	pen_root: Spatial,
	queue: InputQueue,
	prev_position: Option<Vec3>,
	pointer_distance: f32,
	signifiers: Lines,
	root: SpatialRef,
	button: Button,
	reparentable: Option<Reparentable>,
	derezzable: Derezzable,
	_button_model: Model,
}
#[allow(dead_code, clippy::large_enum_variant)]
pub enum Input {
	Grab(GrabInput),
	Pen(PenInput),
}
pub struct GrabInput {
	move_action: SingleAction,
	_field: Field,
	field_spatial: Spatial,
	queue: InputQueue,
	prev_position: Option<Vec3>,
	pointer_distance: f32,
	signifiers: Lines,
	velocity_space: SpatialRef,
	button_hand: Option<ModeButton>,
	button_controller: Option<ModeButton>,
}

impl Input {
	pub async fn new_pen(client: &Client<impl ClientHandler>) -> stardust_xr_fusion::Result<Self> {
		PenInput::new(client).await.map(Input::Pen)
	}
	pub async fn new_grab(client: &Client<impl ClientHandler>) -> stardust_xr_fusion::Result<Self> {
		let hmd = Tracked::hmd_spatial(client).await?;
		let (field_spatial, field_spatial_ref) =
			Spatial::new(client, &hmd, Transform::IDENTITY).await?;
		let (field, _) =
			Field::new(client, &field_spatial, Shape::Sphere { radius: 0.001 }).await?;
		let queue = InputQueue::new(
			client,
			field_spatial.clone(),
			field.clone(),
			field_spatial_ref,
		)
		.await?;
		Ok(Input::Grab(GrabInput {
			signifiers: Lines::new(client, &field_spatial, Vec::new()).await?,
			move_action: SingleAction::default(),
			_field: field,
			queue,
			prev_position: None,
			pointer_distance: 0.0,
			velocity_space: client.root().clone(),
			button_hand: None,
			button_controller: None,
			field_spatial,
		}))
	}
}
impl Input {
	pub fn update_mode(&mut self) -> bool {
		match self {
			Input::Grab(grab_input) => grab_input.update_mode(),
			Input::Pen(pen_input) => pen_input.update_mode(),
		}
	}
	pub async fn handle_input(&mut self, client: &Client<impl ClientHandler>) {
		match self {
			Input::Grab(grab_input) => grab_input.handle_input(),
			Input::Pen(pen_input) => pen_input.handle_input(client).await,
		}
	}
	pub async fn waft(&mut self, delta_secs: f32) -> Vec3 {
		match self {
			Input::Grab(grab_input) => grab_input.waft(delta_secs).await,
			Input::Pen(pen_input) => pen_input.waft(delta_secs).await,
		}
	}
	pub fn update_signifiers(&self, mode: Mode) {
		match self {
			Input::Grab(grab_input) => grab_input.update_signifiers(mode),
			Input::Pen(pen_input) => pen_input.update_signifiers(mode),
		}
	}
	pub fn get_velocity_space(&self) -> SpatialRef {
		match self {
			Input::Grab(grab_input) => grab_input.velocity_space.clone(),
			Input::Pen(pen_input) => pen_input.root.clone(),
		}
	}
}
impl PenInput {
	const LENGTH: f32 = 0.075;
	const THICKNESS: f32 = 0.005;
	fn update_mode(&mut self) -> bool {
		if !self.button.handle_events() {
			return false;
		}
		self.button.released()
	}
	async fn new(client: &Client<impl ClientHandler>) -> stardust_xr_fusion::Result<Self> {
		let root = client.root().clone();
		let (pen_root, pen_root_ref) = Spatial::new(client, &root, Transform::IDENTITY).await?;
		let signifiers = Lines::new(client, &pen_root, Vec::new()).await?;
		let (field_spatial, field_spatial_ref) = Spatial::new(
			client,
			&pen_root_ref,
			Transform::from_translation([0.0, Self::LENGTH * 0.5, 0.0]),
		)
		.await?;
		let (field, _) = Field::new(
			client,
			&field_spatial,
			Shape::Cylinder {
				length: Self::LENGTH,
				radius: Self::THICKNESS * 0.5,
			},
		)
		.await?;
		let queue = InputQueue::new(
			client,
			field_spatial.clone(),
			field.clone(),
			field_spatial_ref.clone(),
		)
		.await?;

		let button = Button::new(
			client,
			&pen_root_ref,
			Transform::from_translation_rotation(
				[0.0, Self::LENGTH * 1.1, 0.0],
				Quat::from_rotation_x(-FRAC_PI_2),
			),
			[0.02; 2].into(),
			ButtonSettings::default(),
		)
		.await?;
		let button_model = Model::new(
			client,
			button.touch_plane().root(),
			Resource::Namespaced {
				namespace: APP_ID.into(),
				path: "move_icon".into(),
			},
		)
		.await?;

		let derezzable = Derezzable::new(client, field_spatial.clone(), field.clone()).await?;
		let mut pen = Self {
			move_action: Default::default(),
			grab_action: Default::default(),
			field,
			pen_root,
			queue,
			prev_position: None,
			pointer_distance: 0.0,
			signifiers,
			root: client.root().clone(),
			button,
			reparentable: None,
			derezzable,
			field_spatial_ref,
			field_spatial,

			_button_model: button_model,
		};
		pen.make_reparentable(client).await;
		Ok(pen)
	}
	async fn make_reparentable(&mut self, client: &Client<impl ClientHandler>) {
		if self.reparentable.is_some() {
			return;
		}
		self.reparentable = Reparentable::new(
			client,
			self.pen_root.clone(),
			self.root.clone(),
			self.field.clone(),
		)
		.await
		.inspect_err(|err| error!("unable to make reparentable: {err}"))
		.ok();
	}
	async fn handle_input(&mut self, client: &Client<impl ClientHandler>) {
		if self.derezzable.receiver.try_recv().is_ok() {
			process::exit(0);
		}
		if !self.queue.handle_events() {
			return;
		}
		self.grab_action.update(
			false,
			&self.queue,
			|data| data.distance() < 0.05,
			|data| match &data.input() {
				InputDataType::Hand { data: _ } => data.datamap_f32("grab_strength") > 0.80,
				InputDataType::Tip { data: _ } => data.datamap_f32("grab") > 0.90,
				InputDataType::Pointer { data: _ } => data.datamap_f32("grab") > 0.90,
			},
		);
		self.move_action
			.update(&self.queue, &|data| match &data.input() {
				// TODO: tune
				InputDataType::Hand { data: _ } => data.datamap_f32("pinch_strength") > 0.9,
				InputDataType::Tip { data: _ } => data.datamap_f32("select") > 0.01,
				InputDataType::Pointer { data: _ } => data.datamap_f32("select") > 0.01,
			});

		if self.grab_action.actor_started() {
			self.reparentable.take();
		}
		if self.grab_action.actor_stopped() {
			self.make_reparentable(client).await;
		}
		let Some(grab_actor) = self.grab_action.actor() else {
			return;
		};
		let transform = match &grab_actor.input() {
			InputDataType::Hand { data: h } => PartialTransform::from_translation_rotation(
				(Vec3::from(h.thumb.tip.pose.position) + Vec3::from(h.index.tip.pose.position))
					* 0.5,
				Quat::from(h.palm.pose.orientation),
			),
			InputDataType::Tip { data: t } => PartialTransform::from_translation_rotation(
				t.pose.position,
				Quat::from(t.pose.orientation) * Quat::from_rotation_x(FRAC_PI_2),
			),
			InputDataType::Pointer { data: p } => {
				if self.grab_action.actor_started() {
					// deepest_point is already a distance along the ray
					self.pointer_distance = p.deepest_point;
				} else {
					self.pointer_distance += (grab_actor.datamap_vec2("scroll_continuous").y * 0.01) + // continuous +Y -> 1cm farther away
						(grab_actor.datamap_vec2("scroll_discrete").y * 0.1); // discrete +Y -> 10cm farther away
				}
				PartialTransform::from_translation_rotation(
					Vec3::from(p.pose.position) + Vec3::from(p.direction()) * self.pointer_distance,
					Quat::from(p.pose.orientation) * Quat::from_rotation_z(-FRAC_PI_4),
				)
			}
		};
		let _ = self
			.pen_root
			.set_relative_transform(self.field_spatial_ref.clone(), transform);
	}
	pub async fn waft(&mut self, _delta_secs: f32) -> Vec3 {
		let Some(grab_actor) = self.grab_action.actor() else {
			self.prev_position = None;
			return Vec3::ZERO;
		};
		let position = match &grab_actor.input() {
			InputDataType::Hand { data: h } => Vec3::from(h.palm.pose.position),
			InputDataType::Tip { data: t } => Vec3::from(t.pose.position),
			InputDataType::Pointer { data: p } => {
				Vec3::from(p.pose.position) + Vec3::from(p.direction()) * self.pointer_distance
			}
		};

		let root_transform = self
			.field_spatial
			.get_relative_transform(self.root.clone())
			.await
			.unwrap()
			.unwrap();
		let mat = mat_from_transform(&root_transform);
		let position = mat.transform_point3(position);
		if self.move_action.currently_acting().contains(grab_actor)
			&& let Some(prev_position) = self.prev_position
		{
			let offset: Vec3 = position - prev_position;
			let offset_magnify = (offset.length()/* * delta_secs */).powf(0.9);
			self.prev_position = Some(position);
			return offset.normalize_or_zero() * offset_magnify;
		}

		self.prev_position = Some(position);

		Vec3::ZERO
	}
	pub fn update_signifiers(&self, mode: Mode) {
		let thickness = Self::THICKNESS * 0.5;
		let visual_length = Self::LENGTH;
		let grabbing = self
			.grab_action
			.actor()
			.is_some_and(|actor| self.move_action.currently_acting().contains(actor));
		let color = match (mode, grabbing) {
			(Mode::Reparent, false) => rgba!(0.015686, 0.992157, 0.298039, 1.0).to_linear(),
			(Mode::MonadoOffset, false) => rgba!(0.361, 0.161, 0.514, 1.0).to_linear(),
			(Mode::Disabled, _) => rgba_linear!(0.033104762, 0.033104762, 0.033104762, 1.),
			(_, true) => rgba_linear!(0., 0.26223028, 1., 1.),
		};
		let signifier_lines = [Line {
			points: vec![
				LinePoint {
					point: [0.0; 3].into(),
					thickness: 0.0,
					color,
				},
				LinePoint {
					point: [0.0, thickness, 0.0].into(),
					thickness,
					color,
				},
				LinePoint {
					point: [0.0, visual_length, 0.0].into(),
					thickness,
					color,
				},
			],
			cyclic: false,
		}];
		self.signifiers.set_lines(&signifier_lines).unwrap();
	}
}
impl GrabInput {
	fn update_mode(&mut self) -> bool {
		self.button_hand.as_mut().is_some_and(|b| b.update())
			|| self.button_controller.as_mut().is_some_and(|b| b.update())
	}
	pub fn handle_input(&mut self) {
		self.queue.handle_events();
		self.move_action.update(
			true,
			&self.queue,
			|_data| true,
			|data| match &data.input() {
				InputDataType::Hand { data: _ } => data.datamap_f32("grab_strength") > 0.9,
				_ => data.datamap_f32("grab") > 0.9,
			},
		);
		let Some(actor) = self.move_action.actor() else {
			return;
		};
		if let InputDataType::Pointer { data: p } = actor.input() {
			if self.move_action.actor_started() {
				// deepest_point is already a distance along the ray
				self.pointer_distance = p.deepest_point;
			} else {
				self.pointer_distance += (actor.datamap_vec2("scroll_continuous").y * 0.01) + // continuous +Y -> 1cm farther away
					(actor.datamap_vec2("scroll_discrete").y * 0.1); // discrete +Y -> 10cm farther away
			}
		}
	}
	pub async fn waft(&mut self, _delta_secs: f32) -> Vec3 {
		let position = self.move_action.actor().map(|p| match &p.input() {
			InputDataType::Hand { data: h } => Vec3::from(h.palm.pose.position),
			InputDataType::Tip { data: t } => Vec3::from(t.pose.position),
			InputDataType::Pointer { data: p } => {
				Vec3::from(p.pose.position) + Vec3::from(p.direction()) * self.pointer_distance
			}
		});

		if let Some(prev_position) = self.prev_position
			&& let Some(position) = position
		{
			let root_transform = self
				.field_spatial
				.get_relative_transform(self.velocity_space.clone())
				.await
				.unwrap()
				.unwrap();
			let mat = mat_from_transform(&root_transform);
			let position = mat.transform_point3(position);

			let offset: Vec3 = position - prev_position;
			let offset_magnify = (offset.length()/* * delta_secs */).powf(0.9);
			self.prev_position = Some(position);
			return offset.normalize_or_zero() * offset_magnify;
		}

		self.prev_position = position;
		Vec3::ZERO
	}
	pub fn update_signifiers(&self, mode: Mode) {
		if matches!(mode, Mode::Disabled) {
			self.signifiers.set_lines(&[]).unwrap();
			return;
		}
		let mut signifier_lines = self
			.move_action
			.hovering()
			.current()
			.iter()
			.map(|input| self.generate_signifier(input, false, mode))
			.collect::<Vec<_>>();
		signifier_lines.extend(
			self.move_action
				.actor()
				.map(|input| self.generate_signifier(input, true, mode)),
		);
		self.signifiers.set_lines(signifier_lines).unwrap();
	}
	fn generate_signifier(&self, input: &InputSnapshot, grabbing: bool, mode: Mode) -> Line {
		let transform = match &input.input() {
			InputDataType::Pointer { data: p } => {
				let distance = if grabbing {
					self.pointer_distance
				} else {
					p.deepest_point
				};
				Mat4::from_rotation_translation(
					p.pose.orientation.into(),
					(Vec3::from(p.pose.position) + Vec3::from(p.direction()) * distance).into(),
				)
			}
			InputDataType::Hand { data: h } => {
				Mat4::from_rotation_translation(
					h.palm.pose.orientation.into(),
					h.palm.pose.position.into(),
				) * Mat4::from_translation(vec3(0.0, 0.05, -0.02))
					* Mat4::from_rotation_x(FRAC_PI_2)
			}
			InputDataType::Tip { data: t } => {
				Mat4::from_rotation_translation(t.pose.orientation.into(), t.pose.position.into())
			}
		};

		let line = circle(
			64,
			0.0,
			match &input.input() {
				InputDataType::Pointer { data: _ } => 0.0025,
				InputDataType::Hand { data: _ } => 0.1,
				InputDataType::Tip { data: _ } => 0.0025,
			},
		)
		.transform(transform);
		if grabbing {
			line.color(rgba_linear!(0., 0.26223028, 1., 1.))
		} else if matches!(mode, Mode::MonadoOffset) {
			line.color(rgba_linear!(1.0, 1.0, 0.0, 1.0))
		} else {
			line
		}
	}
}
