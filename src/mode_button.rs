use std::f32::consts::{FRAC_PI_2, PI};

use glam::Quat;
use stardust_xr_fusion::{
	client::{Client, ClientHandler},
	drawable::{Model, ModelExt as _},
	spatial::{Spatial, SpatialExt, Transform},
	types::Resource,
};
use stardust_xr_molecules::{
	UIElement,
	button::{Button, ButtonSettings},
};
use std::sync::mpsc;

use crate::APP_ID;

struct ButtonEnabled(bool);

pub struct ModeButton {
	button: Button,
	model: Model,
	enabled_event: mpsc::Receiver<ButtonEnabled>,
}

pub enum ButtonLocation {
	Hand,
	Controller,
}

// TODO: properly port at some point maybe?
impl ModeButton {
	pub fn update(&mut self) -> bool {
		self.button.handle_events();
		while let Ok(ButtonEnabled(enabled)) = self.enabled_event.try_recv() {
			// _ = self.button.touch_plane().set_enabled(enabled);
			// _ = self.model.set_enabled(enabled);
		}
		self.button.released()
	}
	pub async fn new(
		client: &Client<impl ClientHandler>,
		location: ButtonLocation,
	) -> Option<Self> {
		// let (dest, spatial_path, tracked_path) = match location {
		// 	ButtonLocation::Hand => (
		// 		"org.stardustxr.Hands",
		// 		"/org/stardustxr/Hand/right/palm",
		// 		"/org/stardustxr/Hand/right",
		// 	),
		// 	ButtonLocation::Controller => (
		// 		"org.stardustxr.Controllers",
		// 		"/org/stardustxr/Controller/right",
		// 		"/org/stardustxr/Controller/right",
		// 	),
		// };
		// let spatial = SpatialRefProxy::new(
		// 	connection,
		// 	WellKnownName::from_static_str(dest).ok()?,
		// 	spatial_path,
		// )
		// .await
		// .ok()?
		// .import(client)
		// .await?;
		let (_, spatial) = Spatial::new(client, client.root(), Transform::IDENTITY)
			.await
			.ok()?;
		let button = Button::new(
			client,
			&spatial,
			match location {
				ButtonLocation::Hand => Transform::from_translation([0.0, -0.02, 0.03]),
				ButtonLocation::Controller => Transform::from_translation_rotation(
					[0.0, 0.01, 0.02],
					Quat::from_rotation_x(PI + FRAC_PI_2),
				),
			},
			[0.02; 2].into(),
			ButtonSettings::default(),
		)
		.await
		.ok()?;

		let model = Model::new(
			client,
			button.touch_plane().root(),
			Resource::Namespaced {
				namespace: APP_ID.into(),
				path: "move_icon".into(),
			},
		)
		.await
		.ok()?;

		let (tx, rx) = mpsc::channel();

		// tokio::spawn(async move {
		// 	if let Ok(is_tracked) = tracked.is_tracked().await {
		// 		_ = tx.send(ButtonEnabled(is_tracked));
		// 	}
		// 	let mut stream = tracked.receive_is_tracked_changed().await;
		// 	while let Some(value) = stream.next().await {
		// 		if let Ok(is_tracked) = value.get().await {
		// 			_ = tx.send(ButtonEnabled(is_tracked));
		// 		}
		// 	}
		// });

		Some(Self {
			button,
			model,
			enabled_event: rx,
		})
	}
}
