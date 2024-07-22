mod root;

use manifest_dir_macros::directory_relative_path;
use root::ClientRoot;
use stardust_xr_fusion::{client::Client, node::NodeType, root::RootAspect};

#[tokio::main(flavor = "current_thread")]
async fn main() {
	color_eyre::install().unwrap();
	let (client, event_loop) = Client::connect_with_async_loop().await.unwrap();
	client
		.set_base_prefixes(&[directory_relative_path!("res")])
		.unwrap();

	let _wrapped = client
		.get_root()
		.alias()
		.wrap(ClientRoot::new(&client).await.unwrap())
		.unwrap();

	tokio::select! {
		_ = tokio::signal::ctrl_c() => (),
		e = event_loop => e.unwrap().unwrap(),
	}
}
