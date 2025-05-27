use std::{net::SocketAddr, sync::Arc, time::Duration};

use log::info;
use tokio::net::UdpSocket;

pub async fn nat_client(addr: SocketAddr) {
    let domain = socket2::Domain::for_address(addr);
    let socket =
        socket2::Socket::new(domain, socket2::Type::DGRAM, Some(socket2::Protocol::UDP)).unwrap();
    socket.set_reuse_port(true).unwrap();
    socket.set_reuse_address(true).unwrap();
    if domain == socket2::Domain::IPV6 {
        socket.set_only_v6(false).unwrap();
    }
    let bind_addr = match domain {
        socket2::Domain::IPV4 => "0.0.0.0:0".parse::<SocketAddr>().unwrap(),
        socket2::Domain::IPV6 => "[::]:0".parse::<SocketAddr>().unwrap(),
        _ => panic!("Unsupported domain"),
    };
    socket.bind(&bind_addr.into()).unwrap();
    socket.set_nonblocking(true).unwrap();

    let sock = Arc::new(UdpSocket::from_std(socket.into()).unwrap());
    let mut buf = [0; 1024];

    sock.send_to(b"ping", addr).await.unwrap();

    let (len, _addr) = loop {
        match tokio::time::timeout(Duration::from_secs(2), sock.recv_from(&mut buf)).await {
            Ok(Ok((len, addr))) => break (len, addr),
            Ok(Err(e)) => {
                unreachable!("error: {:?}", e);
            }
            Err(_) => {
                sock.send_to(b"get", addr).await.unwrap();
                continue;
            }
        }
    };

    let msg = String::from_utf8(buf[..len].to_vec()).unwrap();

    // here we can get the nat address from stun server
    // We can obtain 255 UDP sockets by adding a random algorithm to this address
    // and attempt to connect simultaneously. As long as one connection is successful,
    // it is sufficient. According to the birthday problem theory, randomly selecting
    // 255 from 2^16 - 1024 can achieve a success rate of 60%+. This is an effective port sniffing method.
    let nat_addr: SocketAddr = msg.parse().unwrap();
    info!("Received address: {}", nat_addr);
    let nat_addr = match domain {
        socket2::Domain::IPV4 => SocketAddr::new(nat_addr.ip().to_canonical(), nat_addr.port()),
        socket2::Domain::IPV6 => nat_addr,
        _ => panic!("Unsupported domain"),
    };

    loop {
        sock.send_to(b"Hello, world!", nat_addr).await.unwrap();
        match tokio::time::timeout(Duration::from_millis(200), sock.recv_from(&mut buf)).await {
            Ok(Ok((len, addr))) => {
                let msg = String::from_utf8(buf[..len].to_vec()).unwrap();
                info!("Received message: {} from {}", msg, addr);
                sock.connect(addr).await.unwrap();
                break;
            }
            Ok(Err(e)) => {
                unreachable!("error: {:?}", e);
            }
            Err(_) => {
                continue;
            }
        }
    }
    // now we can send and receive message, nat traversal is done
    // but we need to get the mtu max size between the two peers
    // we can use some packet fragmentation to get the mtu size:
    //
    // 1. Next, detect mtu and convert the protocol：
    //     Quickly detect the bidirectional MSS/MTU using binary search,
    //     dividing from 512 to 1500 to quickly find the target. For example,
    //     from 512 to 1500, send 10 packets simultaneously, evenly distributed
    //     in size from 512 to 1500, and then the other end returns the largest
    //     one received (during this process, both sides record the maximum packet
    //     size they can receive in the next probe packet), iterating until the
    //     bidirectional values stabilize, then take the minimum among them.
    //     If done quickly, it feels like 4 TTLs are enough to stabilize.
    //
    //     Above approach belongs to MTU sniffing of private protocols.
    //     If you need to consider directly switching to the KCP protocol,
    //     you need to take into account the message packet format used by KCP,
    //     and then send invalid commands to allow KCP implementations that are
    //     not currently in use to ignore this message, thus achieving compatibility.
    //
    // 2. If the protocol(such as s2n-quic) cannot be converted directly through the already punched udp socket,
    //    the following solution can be used:
    //     ```rust
    //      let local_addr = sock.local_addr().unwrap();
    //      let remote_addr = sock.peer_addr().unwrap();
    //      drop(sock)
    //      let stream = { // rebind local_addr and connect to remote_addr }
    //     ```
    sock.send(b"yes").await.unwrap();
    let len = sock.recv(&mut buf).await.unwrap();
    let msg = String::from_utf8(buf[..len].to_vec()).unwrap();
    info!(
        "Received message: {} from {}",
        msg,
        sock.peer_addr().unwrap()
    );
}
