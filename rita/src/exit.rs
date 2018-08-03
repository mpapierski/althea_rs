#![cfg_attr(
    feature = "system_alloc",
    feature(alloc_system, allocator_api)
)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

#[cfg(feature = "system_alloc")]
extern crate alloc_system;

#[cfg(feature = "system_alloc")]
use alloc_system::System;

#[cfg(feature = "system_alloc")]
#[global_allocator]
static A: System = System;

extern crate diesel;
#[macro_use]
extern crate failure;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

extern crate actix;
extern crate actix_web;
extern crate byteorder;
extern crate bytes;
extern crate clu;
extern crate docopt;
extern crate dotenv;
extern crate env_logger;
extern crate eui48;
extern crate futures;
extern crate handlebars;
extern crate ipnetwork;
extern crate lettre;
extern crate lettre_email;
extern crate minihttpse;
extern crate rand;
extern crate regex;
extern crate reqwest;
extern crate serde;
extern crate settings;
extern crate tokio;
extern crate trust_dns_resolver;

use settings::{RitaCommonSettings, RitaExitSettings, RitaExitSettingsStruct};

use docopt::Docopt;
#[cfg(not(test))]
use settings::FileWrite;

use actix::registry::SystemService;
use actix::*;
use actix_web::http::Method;
use actix_web::*;

extern crate althea_kernel_interface;
extern crate althea_types;
extern crate babel_monitor;
extern crate exit_db;
extern crate num256;

pub mod actix_utils;
mod middleware;
mod rita_common;
mod rita_exit;

use rita_common::dashboard::network_endpoints::*;
use rita_common::network_endpoints::*;
use rita_exit::network_endpoints::*;

use std::sync::{Arc, RwLock};

#[cfg(test)]
use std::sync::Mutex;

#[derive(Debug, Deserialize)]
struct Args {
    flag_config: String,
    flag_future: bool,
}

lazy_static! {
    static ref USAGE: String = format!(
        "Usage: rita_exit --config=<settings>
Options:
    -c, --config=<settings>   Name of config file
    --future                    Enable B side of A/B releases
About:
    Version {}
    git hash {}",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH")
    );
}

use althea_kernel_interface::KernelInterface;

#[cfg(not(test))]
use althea_kernel_interface::LinuxCommandRunner;
#[cfg(test)]
use althea_kernel_interface::TestCommandRunner;

#[cfg(test)]
lazy_static! {
    pub static ref KI: Box<KernelInterface> = Box::new(TestCommandRunner {
        run_command: Arc::new(Mutex::new(Box::new(|_program, _args| {
            panic!("kernel interface used before initialized");
        })))
    });
}

#[cfg(not(test))]
lazy_static! {
    pub static ref KI: Box<KernelInterface> = Box::new(LinuxCommandRunner {});
}

#[cfg(not(test))]
lazy_static! {
    pub static ref SETTING: Arc<RwLock<RitaExitSettingsStruct>> = {
        let args: Args = Docopt::new((*USAGE).as_str())
            .and_then(|d| d.deserialize())
            .unwrap_or_else(|e| e.exit());

        let settings_file = args.flag_config;

        let s = RitaExitSettingsStruct::new_watched(&settings_file).unwrap();

        s.set_future(args.flag_future);

        clu::exit_init("linux", s.clone());

        s.read().unwrap().write(&settings_file).unwrap();

        s
    };
}

#[cfg(test)]
lazy_static! {
    pub static ref SETTING: Arc<RwLock<RitaExitSettingsStruct>> =
        { Arc::new(RwLock::new(RitaExitSettingsStruct::default())) };
}

fn main() {
    env_logger::init();

    let args: Args = Docopt::new((*USAGE).as_str())
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let settings_file = args.flag_config;

    // to get errors before lazy static
    RitaExitSettingsStruct::new(&settings_file).expect("Settings parse failure");

    trace!("Starting");
    info!(
        "crate ver {}, git hash {}",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH")
    );
    trace!("Starting with Identity: {:?}", SETTING.get_identity());

    let system = actix::System::new(format!("main {}", SETTING.get_network().own_ip));

    assert!(rita_common::debt_keeper::DebtKeeper::from_registry().connected());
    assert!(rita_common::payment_controller::PaymentController::from_registry().connected());
    assert!(rita_common::tunnel_manager::TunnelManager::from_registry().connected());
    assert!(rita_common::http_client::HTTPClient::from_registry().connected());
    assert!(rita_common::traffic_watcher::TrafficWatcher::from_registry().connected());
    assert!(rita_common::peer_listener::PeerListener::from_registry().connected());

    assert!(rita_exit::traffic_watcher::TrafficWatcher::from_registry().connected());
    assert!(rita_exit::db_client::DbClient::from_registry().connected());

    server::new(|| App::new().resource("/hello", |r| r.method(Method::POST).with(hello_response)))
        .bind(format!("[::0]:{}", SETTING.get_network().rita_hello_port))
        .unwrap()
        .shutdown_timeout(0)
        .start();
    server::new(|| {
        App::new().resource("/make_payment", |r| {
            r.method(Method::POST).with(make_payments)
        })
    }).workers(1)
    .bind(format!("[::0]:{}", SETTING.get_network().rita_contact_port))
    .unwrap()
    .shutdown_timeout(0)
    .start();

    // Exit stuff
    server::new(|| {
        App::new()
            .resource("/setup", |r| r.method(Method::POST).with(setup_request))
            .resource("/status", |r| {
                r.method(Method::POST).with_async(status_request)
            }).resource("/list", |r| r.method(Method::POST).with(list_clients))
            .resource("/exit_info", |r| {
                r.method(Method::GET).with(get_exit_info_http)
            }).resource("/rtt", |r| r.method(Method::GET).with(rtt))
    }).bind(format!(
        "[::0]:{}",
        SETTING.get_exit_network().exit_hello_port
    )).unwrap()
    .shutdown_timeout(0)
    .start();

    // Dashboard
    server::new(|| {
        App::new()
            .middleware(middleware::Headers)
            // assuming exit nodes dont need wifi
            //.resource("/wifisettings", |r| r.route().filter(pred::Get()).h(get_wifi_config))
            //.resource("/wifisettings", |r| r.route().filter(pred::Post()).h(set_wifi_config))
            .route("/info", Method::GET, get_own_info)
            .route("/settings", Method::GET, get_settings)
            .route("/settings", Method::POST, set_settings)
            .route("/version", Method::GET, version)
    }).bind(format!(
        "[::0]:{}",
        SETTING.get_network().rita_dashboard_port
    )).unwrap()
    .shutdown_timeout(0)
    .start();

    let common = rita_common::rita_loop::RitaLoop::new();
    let _: Addr<_> = common.start();

    let exit = rita_exit::rita_loop::RitaLoop {};
    let _: Addr<_> = exit.start();

    system.run();
}
