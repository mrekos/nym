// Copyright 2020 - Nym Technologies SA <contact@nymtech.net>
// SPDX-License-Identifier: Apache-2.0

use crate::commands::*;
use crate::config::Config;
use crate::node::Gateway;
use clap::{App, Arg, ArgMatches};
use config::NymConfig;
use log::*;

pub fn command_args<'a, 'b>() -> clap::App<'a, 'b> {
    let app = App::new("run")
        .about("Starts the gateway")
        .arg(
            Arg::with_name(ID_ARG_NAME)
                .long(ID_ARG_NAME)
                .help("Id of the gateway we want to run")
                .takes_value(true)
                .required(true),
        )
        // the rest of arguments are optional, they are used to override settings in config file
        .arg(
            Arg::with_name(HOST_ARG_NAME)
                .long(HOST_ARG_NAME)
                .help("The custom host on which the gateway will be running for receiving sphinx packets")
                .takes_value(true)
        )
        .arg(
            Arg::with_name(MIX_PORT_ARG_NAME)
                .long(MIX_PORT_ARG_NAME)
                .help("The port on which the gateway will be listening for sphinx packets")
                .takes_value(true)
        )
        .arg(
            Arg::with_name(CLIENTS_PORT_ARG_NAME)
                .long(CLIENTS_PORT_ARG_NAME)
                .help("The port on which the gateway will be listening for clients gateway-requests")
                .takes_value(true)
        )
        .arg(
            Arg::with_name(ANNOUNCE_HOST_ARG_NAME)
                .long(ANNOUNCE_HOST_ARG_NAME)
                .help("The host that will be reported to the directory server")
                .takes_value(true),
        )
        .arg(
            Arg::with_name(DATASTORE_PATH)
                .long(DATASTORE_PATH)
                .help("Path to sqlite database containing all gateway persistent data")
                .takes_value(true)
        )
        .arg(
            Arg::with_name(VALIDATOR_APIS_ARG_NAME)
                .long(VALIDATOR_APIS_ARG_NAME)
                .help("Comma separated list of endpoints of the validators APIs")
                .takes_value(true),
        )
        .arg(
            Arg::with_name(TESTNET_MODE_ARG_NAME)
                .long(TESTNET_MODE_ARG_NAME)
                .help("Set this gateway to work in a testnet mode that would allow clients to bypass bandwidth credential requirement")
        );

    #[cfg(not(feature = "coconut"))]
        let app = app
        .arg(Arg::with_name(ETH_ENDPOINT)
            .long(ETH_ENDPOINT)
            .help("URL of an Ethereum full node that we want to use for getting bandwidth tokens from ERC20 tokens")
            .takes_value(true))
        .arg(Arg::with_name(VALIDATORS_ARG_NAME)
            .long(VALIDATORS_ARG_NAME)
            .help("Comma separated list of endpoints of the validator")
            .takes_value(true))
        .arg(Arg::with_name(COSMOS_MNEMONIC)
            .long(COSMOS_MNEMONIC)
            .help("Cosmos wallet mnemonic")
            .takes_value(true));

    app
}

fn show_binding_warning(address: String) {
    println!("\n##### NOTE #####");
    println!(
        "\nYou are trying to bind to {} - you might not be accessible to other nodes\n\
         You can ignore this warning if you're running setup on a local network \n\
         or have set a custom 'announce-host'",
        address
    );
    println!("\n\n");
}

fn special_addresses() -> Vec<&'static str> {
    vec!["localhost", "127.0.0.1", "0.0.0.0", "::1", "[::1]"]
}

pub async fn execute(matches: ArgMatches<'static>) {
    let id = matches.value_of(ID_ARG_NAME).unwrap();

    println!("Starting gateway {}...", id);

    let mut config = match Config::load_from_file(Some(id)) {
        Ok(cfg) => cfg,
        Err(err) => {
            error!("Failed to load config for {}. Are you sure you have run `init` before? (Error was: {})", id, err);
            return;
        }
    };

    config = override_config(config, &matches);

    if !version_check(&config) {
        error!("failed the local version check");
        return;
    }

    if special_addresses().contains(&&*config.get_listening_address().to_string()) {
        show_binding_warning(config.get_listening_address().to_string());
    }

    let mut gateway = Gateway::new(config).await;
    println!(
        "\nTo bond your gateway you will need to install the Nym wallet, go to https://nymtech.net/get-involved and select the Download button.\n\
         Select the correct version and install it to your machine. You will need to provide the following: \n ");
    gateway.print_node_details();

    gateway.run().await;
}
