use althea_types::{LocalIdentity, PaymentTx};

use actix::registry::SystemService;
use actix_web::*;

use futures::Future;

use failure::Error;

use settings::RitaCommonSettings;
use SETTING;

use std::net::SocketAddr;

use rita_common;
use rita_common::payment_controller::PaymentController;
use rita_common::peer_listener::Peer;
use rita_common::tunnel_manager::{IdentityCallback, TunnelManager};

use std::boxed::Box;

#[derive(Serialize)]
pub struct JsonStatusResponse {
    response: String,
}

impl JsonStatusResponse {
    pub fn new(ret_val: Result<String, Error>) -> Result<Json<JsonStatusResponse>, Error> {
        let res_string = match ret_val {
            Ok(msg) => msg.clone(),
            Err(e) => format!("{}", e),
        };

        Ok(Json(JsonStatusResponse {
            response: res_string,
        }))
    }
}

pub fn make_payments(
    pmt: (Json<PaymentTx>, HttpRequest),
) -> Box<Future<Item = HttpResponse, Error = Error>> {
    info!("Got Payment from {:?}", pmt.1.connection_info().remote());
    trace!("Received payment: {:?}", pmt.0);
    PaymentController::from_registry()
        .send(rita_common::payment_controller::PaymentReceived(
            pmt.0.clone(),
        ))
        .from_err()
        .and_then(|_| Ok(HttpResponse::Ok().into()))
        .responder()
}

pub fn hello_response(
    req: (Json<LocalIdentity>, HttpRequest),
) -> Box<Future<Item = Json<LocalIdentity>, Error = Error>> {
    let their_id = req.0.clone();

    let socket = req
        .1
        .connection_info()
        .remote()
        .unwrap()
        .parse::<SocketAddr>()
        .unwrap();

    info!("Got Hello from {:?}", req.1.connection_info().remote());

    trace!("Received neighbour identity: {:?}", their_id);

    info!("opening tunnel in hello_response for {:?}", their_id);

    let peer = Peer {
        contact_ip: socket.ip(),
        contact_socket: socket,
        ifidx: 0, // only works because we lookup ifname in kernel interface
    };

    // We send the callback, which can safely allocate a port because it already successfully
    // contacted a neighbor
    TunnelManager::from_registry()
        .send(IdentityCallback(their_id, peer, None))
        .and_then(|tunnel| {
            Ok(Json(LocalIdentity {
                global: SETTING.get_identity(),
                wg_port: tunnel.unwrap().listen_port,
            }))
        })
        .from_err()
        .responder()
}

pub fn version(_req: HttpRequest) -> String {
    format!(
        "crate ver {}\ngit hash {}",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH")
    )
}
