use clap::App;
use log::{error, info};

use client_core::client::key_manager::KeyManager;
use client_core::config::persistence::key_pathfinder::ClientKeyPathfinder;
use config::NymConfig;
use nym_client::client::NymClient as NativeNymClient;
use nym_socks5::client::NymClient as Socks5NymClient;
use nymsphinx::addressing::clients::Recipient;
use rand::rngs::OsRng;

#[cfg(not(feature = "coconut"))]
use nym_client::commands::{DEFAULT_ETH_ENDPOINT, DEFAULT_ETH_PRIVATE_KEY};

static NATIVE_CLIENT_CONFIG_ID: &str = "hrycyszynvpn";
static SOCKS5_CONFIG_ID: &str = "hrycyszynvpn";

// static NATIVE_CLIENT_CONFIG_ID: &str = "test101";
// static SOCKS5_CONFIG_ID: &str = "test102";

static GATEWAY_ID: &str = "83x9YyNkQ5QEY84ZU6Wmq8XHqfwf9SUtR7g5PAYB1FRY"; // sandbox

#[tokio::main]
async fn main() {
  println!(
    r#"
 _                                                                
| |                                                               
| |__  _ __ _   _  ___ _   _ ___ _____   _ _ ____   ___ __  _ __  
| '_ \| '__| | | |/ __| | | / __|_  / | | | '_ \ \ / / '_ \| '_ \ 
| | | | |  | |_| | (__| |_| \__ \/ /| |_| | | | \ V /| |_) | | | |
|_| |_|_|   \__, |\___|\__, |___/___|\__, |_| |_|\_/ | .__/|_| |_|
             __/ |      __/ |         __/ |          | |          
            |___/      |___/         |___/           |_|          
"#
  );

  setup_logging();

  let arg_matches = App::new("Nym hrycyszynvpn")
    .version(env!("CARGO_PKG_VERSION"))
    .author("Nymtech")
    .about("A Socks5 localhost proxy that uses the Nym mixnet to proxy traffic securely")
    .subcommand(App::new("init").about("Initialise the config first"))
    .get_matches();

  match arg_matches.subcommand() {
    ("init", Some(_m)) => init().await,
    _ => run().await,
  }
}

async fn init() {
  println!("Initialising...");

  let native_client_address = init_native_client(GATEWAY_ID).await;
  init_socks5(native_client_address, GATEWAY_ID).await;

  println!("Configuration saved 🚀");
}

async fn init_native_client(chosen_gateway_id: &str) -> Recipient {
  let mut config = nym_client::client::config::Config::new(NATIVE_CLIENT_CONFIG_ID);

  let mut rng = OsRng;

  // create identity, encryption and ack keys.
  let mut key_manager = KeyManager::new(&mut rng);

  let gateway_details = nym_client::commands::init::gateway_details(
    config.get_base().get_validator_api_endpoints(),
    Some(chosen_gateway_id),
  )
  .await;

  config
    .get_base_mut()
    .with_gateway_id(gateway_details.identity_key.to_base58_string());

  config.get_base_mut().with_testnet_mode(true);
  config
    .get_base_mut()
    .with_eth_endpoint(DEFAULT_ETH_ENDPOINT);
  config
    .get_base_mut()
    .with_eth_private_key(DEFAULT_ETH_PRIVATE_KEY);

  let shared_keys = nym_client::commands::init::register_with_gateway(
    &gateway_details,
    key_manager.identity_keypair(),
  )
  .await;

  config
    .get_base_mut()
    .with_gateway_listener(gateway_details.clients_address());
  key_manager.insert_gateway_shared_key(shared_keys);

  let pathfinder = ClientKeyPathfinder::new_from_config(config.get_base());
  key_manager
    .store_keys(&pathfinder)
    .expect("Failed to generated keys");
  println!("Saved all generated keys");

  let config_save_location = config.get_config_file_save_location();
  config
    .save_to_file(None)
    .expect("Failed to save the config file");
  println!("Saved configuration file to {:?}", config_save_location);
  println!("Using gateway: {}", config.get_base().get_gateway_id(),);
  println!("Client configuration completed.\n\n\n");

  nym_client::commands::init::show_address(&config)
}

async fn init_socks5(provider_address: Recipient, chosen_gateway_id: &str) {
  let id = SOCKS5_CONFIG_ID;

  let mut config = nym_socks5::client::config::Config::new(id, &format!("{}", provider_address));

  let mut rng = OsRng;

  // create identity, encryption and ack keys.
  let mut key_manager = KeyManager::new(&mut rng);

  config.get_base_mut().with_testnet_mode(true);
  config
    .get_base_mut()
    .with_eth_endpoint(DEFAULT_ETH_ENDPOINT);
  config
    .get_base_mut()
    .with_eth_private_key(DEFAULT_ETH_PRIVATE_KEY);

  let gateway_details = nym_client::commands::init::gateway_details(
    config.get_base().get_validator_api_endpoints(),
    Some(chosen_gateway_id),
  )
  .await;
  config
    .get_base_mut()
    .with_gateway_id(gateway_details.identity_key.to_base58_string());
  let shared_keys = nym_client::commands::init::register_with_gateway(
    &gateway_details,
    key_manager.identity_keypair(),
  )
  .await;

  config
    .get_base_mut()
    .with_gateway_listener(gateway_details.clients_address());
  key_manager.insert_gateway_shared_key(shared_keys);

  let pathfinder = ClientKeyPathfinder::new_from_config(config.get_base());
  key_manager
    .store_keys(&pathfinder)
    .expect("Failed to generated keys");
  println!("Saved all generated keys");

  let config_save_location = config.get_config_file_save_location();
  config
    .save_to_file(None)
    .expect("Failed to save the config file");
  println!("Saved configuration file to {:?}", config_save_location);
  println!("Using gateway: {}", config.get_base().get_gateway_id(),);
  println!("Client configuration completed.\n\n\n");
}

async fn run() {
  let nym_client = start_nym_native_client().await;
  if nym_client.is_none() {
    return;
  }

  let address = nym_client.unwrap().as_mix_recipient();
  info!("Nym client address is {}", address);

  let socks5_client = start_nym_socks5_client(&address).await;
  if socks5_client.is_none() {
    return;
  }

  let _requester = start_network_requester().await;

  info!("✅ SUCCESS - to exit press Ctrl+C...");

  wait_for_exit().await;
}

async fn start_nym_native_client() -> Option<NativeNymClient> {
  let id = NATIVE_CLIENT_CONFIG_ID;

  let config = match nym_client::client::config::Config::load_from_file(Some(id)) {
    Ok(cfg) => cfg,
    Err(err) => {
      error!(
        "Failed to load config for {}. Are you sure you have run `init` before? (Error was: {})",
        id, err
      );
      return None;
    }
  };

  // let base_config = config.get_base_mut();
  // base_config.with_gateway_id("83x9YyNkQ5QEY84ZU6Wmq8XHqfwf9SUtR7g5PAYB1FRY");

  let mut nym_client = NativeNymClient::new(config);
  nym_client.start().await;
  Some(nym_client)
}

async fn start_nym_socks5_client(recipient: &Recipient) -> Option<Socks5NymClient> {
  let id = SOCKS5_CONFIG_ID;

  let mut config = match nym_socks5::client::config::Config::load_from_file(Some(id)) {
    Ok(cfg) => cfg,
    Err(err) => {
      error!(
        "Failed to load config for {}. Are you sure you have run `init` before? (Error was: {})",
        id, err
      );
      return None;
    }
  };

  // let base_config = config.get_base_mut();
  // base_config.with_gateway_id("83x9YyNkQ5QEY84ZU6Wmq8XHqfwf9SUtR7g5PAYB1FRY");

  config = config.with_provider_mix_address(recipient.to_string());

  let mut nym_client = Socks5NymClient::new(config);
  nym_client.start().await;
  Some(nym_client)
}

async fn start_network_requester() -> nym_network_requester::core::ServiceProvider {
  let open_proxy = true;
  let uri = "ws://localhost:1977";
  println!("Starting socks5 service provider:");
  let mut server = nym_network_requester::core::ServiceProvider::new(uri.into(), open_proxy);
  server.run().await;
  server
}

async fn wait_for_exit() {
  if let Err(e) = tokio::signal::ctrl_c().await {
    error!(
      "There was an error while capturing SIGINT - {:?}. We will terminate regardless",
      e
    );
  }

  println!(
    "Received SIGINT - the client will terminate now (threads are not yet nicely stopped, if you see stack traces that's alright)."
  );
}

fn setup_logging() {
  let mut log_builder = pretty_env_logger::formatted_timed_builder();
  if let Ok(s) = ::std::env::var("RUST_LOG") {
    log_builder.parse_filters(&s);
  } else {
    // default to 'Info'
    log_builder.filter(None, log::LevelFilter::Info);
  }

  log_builder
    .filter_module("hyper", log::LevelFilter::Warn)
    .filter_module("tokio_reactor", log::LevelFilter::Warn)
    .filter_module("reqwest", log::LevelFilter::Warn)
    .filter_module("mio", log::LevelFilter::Warn)
    .filter_module("want", log::LevelFilter::Warn)
    .filter_module("tungstenite", log::LevelFilter::Warn)
    .filter_module("tokio_tungstenite", log::LevelFilter::Warn)
    .init();
}
