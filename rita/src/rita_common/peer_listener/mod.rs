/*
PeerListener is used to detect nearby mesh peers, it listens on a ff02::/8 ipv6 address, which is
a link local multicast address, on each listen port. 

On initilization a set of ListenInterface objects are created, these are important becuase they
actually hold the sockets required to listen and broadcast on the listen interfaces, every
rita_loop iteration we send out out own IP as a UDP boradcast packet and then get our peers
off the queue. These are turned into Peer structs which are passed to TunnelManager to do
whatever remaining work there may be. 
*/
use actix::prelude::*;
use actix::{Actor, Context};
use failure::Error;
use settings::RitaCommonSettings;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6, UdpSocket};

use rita_common::rita_loop::Tick;

use KI;
use SETTING;

mod message;
use self::message::PeerMessage;

#[derive(Debug)]
pub struct PeerListener {
    interfaces: Vec<ListenInterface>,
    peers: Vec<Peer>,
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub contact_ip: IpAddr,
    pub ifidx: u32,
    pub contact_socket: SocketAddr,
}

impl Peer {
    pub fn new(ip: Ipv6Addr, idx: u32) -> Peer {
        let port = SETTING.get_network().rita_hello_port;
        let socket = SocketAddrV6::new(ip, port.into(), 0, idx);
        Peer {
            contact_ip: ip.into(),
            ifidx: idx,
            contact_socket: socket.into(),
        }
    }
}

impl Actor for PeerListener {
    type Context = Context<Self>;
}

impl Default for PeerListener {
    fn default() -> PeerListener {
        PeerListener::new().unwrap()
    }
}

impl PeerListener {
    pub fn new() -> Result<PeerListener, Error> {
        Ok(PeerListener {
            interfaces: Vec::new(),
            peers: Vec::new(),
        })
    }
}

impl Supervised for PeerListener {}

impl SystemService for PeerListener {
    // Binds to all ready interfaces
    fn service_started(&mut self, _ctx: &mut Context<Self>) {
        info!("PeerListener starting");
        let interfaces = SETTING.get_network().peer_interfaces.clone();
        let iface_list = interfaces;
        for iface in iface_list.iter() {
            let res = ListenInterface::new(iface);
            if res.is_ok() {
                self.interfaces.push(res.unwrap());
            }
        }
    }
}

impl Handler<Tick> for PeerListener {
    type Result = Result<(), Error>;
    fn handle(&mut self, _: Tick, _ctx: &mut Context<Self>) -> Self::Result {
        trace!("Starting PeerListener tick!");
        let res = send_imhere(&mut self.interfaces);
        if res.is_err() {
            error!("Sending ImHere failed with {:?}", res);
        }

        match recieve_imhere(&mut self.interfaces) {
            Ok(new_peers) => {
                self.peers = new_peers;
            }
            Err(e) => {
                error!("Receiving ImHere failed with {:?}", e);
            }
        }

        Ok(())
    }
}

pub struct Listen(pub String);
impl Message for Listen {
    type Result = ();
}

impl Handler<Listen> for PeerListener {
    type Result = ();

    fn handle(&mut self, listen: Listen, _: &mut Context<Self>) -> Self::Result {
        trace!("Peerlistener listen on {:?}", listen.0);
        let new_iface_name = listen.0;
        let new_iface = ListenInterface::new(&new_iface_name);
        match new_iface {
            Ok(n) => {
                self.interfaces.push(n);
                SETTING
                    .get_network_mut()
                    .peer_interfaces
                    .insert(new_iface_name);
            }
            Err(e) => {
                error!("Peer listener failed to listen on {:?}", e);
            }
        }
    }
}

pub struct UnListen(pub String);
impl Message for UnListen {
    type Result = ();
}

impl Handler<UnListen> for PeerListener {
    type Result = ();

    fn handle(&mut self, un_listen: UnListen, _: &mut Context<Self>) -> Self::Result {
        trace!("Peerlistener unlisten on {:?}", un_listen.0);
        let ifname_to_delete = un_listen.0;
        let mut entry_found = false;
        let mut to_del = 0;
        let mut count = 0;
        for item in self.interfaces.iter() {
            if item.ifname == ifname_to_delete {
                to_del = count;
                entry_found = true;
            }
            count = count + 1;
        }
        if entry_found {
            self.interfaces.remove(to_del);
            SETTING
                .get_network_mut()
                .peer_interfaces
                .remove(&ifname_to_delete);
        } else {
            error!("Peer listener failed to unlisten on {:?}", ifname_to_delete);
        }
    }
}

pub struct GetListen;
impl Message for GetListen {
    type Result = Result<HashSet<String>, Error>;
}

impl Handler<GetListen> for PeerListener {
    type Result = Result<HashSet<String>, Error>;
    fn handle(&mut self, _: GetListen, _: &mut Context<Self>) -> Self::Result {
        let mut res = HashSet::new();
        for item in self.interfaces.iter() {
            res.insert(item.ifname.clone());
        }
        Ok(res.clone())
    }
}

#[derive(Debug)]
pub struct GetPeers();
impl Message for GetPeers {
    type Result = Result<Vec<Peer>, Error>;
}

impl Handler<GetPeers> for PeerListener {
    type Result = Result<Vec<Peer>, Error>;

    fn handle(&mut self, _: GetPeers, _: &mut Context<Self>) -> Self::Result {
        Ok(self.peers.clone())
    }
}

#[derive(Debug)]
pub struct ListenInterface {
    ifname: String,
    ifidx: u32,
    multicast_socketaddr: SocketAddrV6,
    multicast_socket: UdpSocket,
    linklocal_socket: UdpSocket,
    linklocal_ip: Ipv6Addr,
}

impl ListenInterface {
    pub fn new(ifname: &str) -> Result<ListenInterface, Error> {
        let port = SETTING.get_network().rita_hello_port;
        let disc_ip = SETTING.get_network().discovery_ip;
        trace!("Binding to {:?} for ListenInterface", ifname);
        // Lookup interface link local ip
        let link_ip = match KI.get_link_local_device_ip(&ifname) {
            Ok(ip) => ip,
            Err(e) => {
                return Err(e);
            }
        };
        // Lookup interface index

        let iface_index = match KI.get_iface_index(&ifname) {
            Ok(idx) => idx,
            Err(_) => 0,
        };
        // Bond to multicast discovery address on each listen port
        let multicast_socketaddr = SocketAddrV6::new(disc_ip, port.into(), 0, iface_index);
        let multicast_socket = UdpSocket::bind(multicast_socketaddr)
            .expect("Failed to bind to peer discovery address!");
        let res = multicast_socket.join_multicast_v6(&disc_ip, iface_index);
        trace!("ListenInterface init set multicast v6 with {:?}", res);
        let res = multicast_socket.set_nonblocking(true);
        trace!(
            "ListenInterface multicast init set nonblocking with {:?}",
            res
        );

        let linklocal_socketaddr = SocketAddrV6::new(link_ip, port.into(), 0, iface_index);
        let linklocal_socket = UdpSocket::bind(linklocal_socketaddr).expect(&format!(
            "ListenInterface Failed to bind to link local address {:?} on {:?} with iface_index {:?} ",
            link_ip, ifname, iface_index
        ));
        let res = linklocal_socket.set_nonblocking(true);
        trace!("ListenInterface init set nonblocking with {:?}", res);

        let res = linklocal_socket.join_multicast_v6(&disc_ip, iface_index);
        trace!("ListenInterface Set link local multicast v6 with {:?}", res);

        Ok(ListenInterface {
            ifname: ifname.to_string(),
            ifidx: iface_index,
            multicast_socket: multicast_socket,
            linklocal_socket: linklocal_socket,
            multicast_socketaddr: multicast_socketaddr,
            linklocal_ip: link_ip,
        })
    }
}

fn send_imhere(interfaces: &mut Vec<ListenInterface>) -> Result<(), Error> {
    trace!("About to send ImHere");
    for listen_interface in interfaces.iter_mut() {
        trace!(
            "Sending ImHere to {:?}, with ip {:?}",
            listen_interface.ifname,
            listen_interface.linklocal_ip
        );
        let message = PeerMessage::ImHere(listen_interface.linklocal_ip.clone());
        let result = listen_interface
            .linklocal_socket
            .send_to(&message.encode(), listen_interface.multicast_socketaddr);
        trace!("Sending ImHere to broadcast gets {:?}", result);
    }
    Ok(())
}

fn recieve_imhere(interfaces: &mut Vec<ListenInterface>) -> Result<Vec<Peer>, Error> {
    trace!("About to dequeue ImHere");
    let mut output = Vec::<Peer>::new();
    for listen_interface in interfaces.iter_mut() {
        let mut socket_empty = false;
        // Since the only datagrams we are interested in are very small (22 bytes plus overhead)
        // this buffer is kept intentionally small to discard larger packets earlier rather than later
        while !socket_empty {
            let mut datagram: [u8; 100] = [0; 100];
            let (res, sock_addr) = match listen_interface.multicast_socket.recv_from(&mut datagram)
            {
                Ok(res) => res,
                Err(e) => {
                    trace!("Could not recv ImHere: {:?}", e);
                    // TODO Consider we might want to remove interfaces that produce specific types
                    // of errors from the active list
                    socket_empty = true;
                    continue;
                }
            };
            trace!(
                "Received {} bytes on multicast socket from {:?}",
                res,
                sock_addr
            );

            let ipaddr = match PeerMessage::decode(&datagram.to_vec()) {
                Ok(PeerMessage::ImHere(ipaddr)) => ipaddr,
                Err(e) => {
                    warn!("ImHere decode failed: {:?}", e);
                    continue;
                }
            };

            if ipaddr == listen_interface.linklocal_ip {
                trace!("Got ImHere from myself");
                continue;
            }

            for peer in output.iter() {
                if peer.contact_ip == ipaddr {
                    trace!(
                        "Discarding ImHere We already have a peer with {:?} for this cycle",
                        ipaddr
                    );
                    continue;
                }
            }
            trace!("ImHere with {:?}", ipaddr);
            let peer = Peer::new(ipaddr, listen_interface.ifidx);
            output.push(peer);
        }
    }
    Ok(output)
}
