mod input;
mod mode_button;
mod monado_movement;
mod solar_sailer;
mod translate_movement;

use gluon_ipc::Liveness;
use input::Input;
use solar_sailer::{Mode, SolarSailer};
use stardust_xr_fusion::{client::Client, project_local_resources};
use tokio::sync::broadcast::error::RecvError;

pub const APP_ID: &str = "org.stardustxr.SolarSailer";

#[tokio::main(flavor = "current_thread")]
async fn main() {
	tracing_subscriber::fmt().pretty().with_file(false).init();
	let (client, _) = Client::connect(&[&project_local_resources!("data")])
		.await
		.unwrap();

	// let mut button_hand = ModeButton::new(&client, ButtonLocation::Hand).await;
	// let mut button_controller = ModeButton::new(&client, ButtonLocation::Controller).await;

	let input = Input::new_pen(&client).await.unwrap();

	let mut solar_sailer = SolarSailer::new(&client, input).await;
	let mut recv = client.frame_receiver();
	let server = client.server();
	loop {
		let info = tokio::select! {
			f = recv.recv() => {
				match f {
					Ok(info) => info,
					Err(RecvError::Closed) => break,
					Err(RecvError::Lagged(_)) => continue,
				}
			}
			_ = server.death_notification() => break,
		};

		solar_sailer.handle_input().await;
		let switch_mode = solar_sailer.should_switch_mode();
		// if switch_mode {
		// 	solar_sailer.mode = match solar_sailer.mode {
		// 		Mode::Disabled => Mode::MonadoOffset,
		// 		Mode::MonadoOffset => Mode::Zone,
		// 		Mode::Zone => Mode::Disabled,
		// 	};
		// }
		if switch_mode {
			solar_sailer.switch_mode(match solar_sailer.current_mode() {
				Mode::Translate => Mode::MonadoOffset,
				Mode::MonadoOffset => Mode::Translate,
				Mode::Disabled => Mode::MonadoOffset,
			});
		}

		solar_sailer.update_signifiers();
		solar_sailer.update_velocity(info.delta).await;
		solar_sailer.apply_offset(&client, info.delta).await;
	}
}
