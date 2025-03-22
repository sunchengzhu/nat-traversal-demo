pub mod tcp;
pub mod udp;

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use futures::{SinkExt, StreamExt};
use log::info;
use tokio::{
    net::{TcpSocket, TcpStream, UdpSocket},
    time,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

const ADDR: &str = "0.0.0.0:8090";
static GLOBAL_STATE: LazyLock<Mutex<HashMap<usize, SocketAddr>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

pub struct StunSession {
    stream: Framed<TcpStream, LengthDelimitedCodec>,
    session_id: usize,
    addr: SocketAddr,
}

impl StunSession {
    pub async fn run(&mut self) {
        let mut finished = false;
        loop {
            if !finished {
                let info = { GLOBAL_STATE.lock().unwrap().clone() };
                for (_, addr) in info.into_iter().filter(|(k, _)| k != &self.session_id) {
                    let remote_info = bytes::Bytes::from(
                        serde_json::json!({ "address": addr.to_string() })
                            .to_string()
                            .into_bytes(),
                    );
                    self.stream.send(remote_info).await.unwrap();
                    finished = true;
                }
            } else {
                match self.stream.next().await {
                    Some(Ok(data)) => {
                        info!(
                            "Tcp Received message: {:?} from {}",
                            String::from_utf8(data.to_vec()).unwrap(),
                            self.addr
                        );
                    }
                    Some(Err(err)) => {
                        info!("Tcp Error: {}", err);
                    }
                    None => {
                        info!("Tcp Connection closed");
                    }
                }
                // cleanup session state
                GLOBAL_STATE.lock().unwrap().remove(&self.session_id);
                break;
            }
            time::sleep(time::Duration::from_secs(1)).await;
        }
    }
}

pub async fn tcp_stun_server() {
    let socket = TcpSocket::new_v4().unwrap();
    socket.set_reuseaddr(true).unwrap();
    socket.set_reuseport(true).unwrap();
    socket.bind(ADDR.parse().unwrap()).unwrap();
    let listener = socket.listen(1024).unwrap();
    info!("Tcp listening on: {}", ADDR);
    let mut next_session_id = 0;

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        info!("Tcp Accepted connection from: {}", addr);
        GLOBAL_STATE.lock().unwrap().insert(next_session_id, addr);
        tokio::spawn(async move {
            StunSession {
                stream: Framed::new(stream, LengthDelimitedCodec::new()),
                session_id: next_session_id,
                addr,
            }
            .run()
            .await;
        });
        next_session_id += 1;
    }
}

pub async fn udp_stun_server() {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .unwrap();
    socket.set_reuse_port(true).unwrap();
    socket.set_reuse_address(true).unwrap();
    socket
        .bind(&ADDR.parse::<SocketAddr>().unwrap().into())
        .unwrap();
    socket.set_nonblocking(true).unwrap();

    info!("Udp listening on: {}", ADDR);

    let sock = Arc::new(UdpSocket::from_std(socket.into()).unwrap());
    let mut buf = [0; 1024];

    let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, SocketAddr)>(8);
    let sock_clone = Arc::clone(&sock);

    tokio::spawn(async move {
        let mut nat_addr: Vec<SocketAddr> = Vec::new();
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        let mut set_time = None;
        loop {
            tokio::select! {
                Some((cmd, addr)) = rx.recv() => {
                    match cmd.as_str() {
                        "ping" => {
                            if !nat_addr.contains(&addr) {
                                nat_addr.push(addr);
                                if set_time.is_none() {
                                    set_time = Some(tokio::time::Instant::now());
                                }
                                info!("Udp NAT address: {:?}", addr);
                            }
                            if nat_addr.len() == 2 {
                                let peer1 = nat_addr[0];
                                let peer2 = nat_addr[1];
                                // exchange peer address
                                sock_clone
                                    .send_to(peer2.to_string().as_bytes(), peer1)
                                    .await
                                    .unwrap();
                                sock_clone
                                    .send_to(peer1.to_string().as_bytes(), peer2)
                                    .await
                                    .unwrap();
                                info!("Udp exchange peer address: {:?} <-> {:?}", peer1, peer2);
                            }
                        }
                        "get" => {
                            for peer in &nat_addr {
                                if peer != &addr {
                                    sock_clone
                                        .send_to(peer.to_string().as_bytes(), addr)
                                        .await
                                        .unwrap();
                                    info!("Udp re-send peer address: {:?} -> {:?}", peer, addr);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ = interval.tick() => {
                    if let Some(time) = set_time {
                        if time.elapsed() > Duration::from_secs(15) {
                            nat_addr.clear();
                            set_time = None;
                            info!("Udp clear NAT address");
                        }
                    }
                }
            }
        }
    });

    loop {
        let (len, addr) = sock.recv_from(&mut buf).await.unwrap();
        info!("Udp {:?} bytes received from {:?}", len, addr);

        tx.send((String::from_utf8_lossy(&buf[..len]).to_string(), addr))
            .await
            .unwrap();
    }
}
