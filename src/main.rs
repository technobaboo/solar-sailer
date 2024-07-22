mod solar_sailer;

use libmonado_rs::Monado;
use manifest_dir_macros::directory_relative_path;
use solar_sailer::SolarSailer;
use stardust_xr_fusion::{client::Client, node::NodeType, root::RootAspect};

#[tokio::main(flavor = "current_thread")]
async fn main() {
	color_eyre::install().unwrap();
	let (client, event_loop) = Client::connect_with_async_loop().await.unwrap();
	client
		.set_base_prefixes(&[directory_relative_path!("res")])
		.unwrap();

	let monado = Monado::auto_connect().expect("Couldn't connect to monado :(");
	let _wrapped = client
		.get_root()
		.alias()
		.wrap(SolarSailer::new(monado, &client, 0.005).unwrap())
		.unwrap();

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e.unwrap().unwrap(),
	}
}
