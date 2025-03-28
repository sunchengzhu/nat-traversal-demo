use std::{collections::HashMap, net::SocketAddr};

use futures::{SinkExt, StreamExt};
use log::info;
use tokio::{net::TcpSocket, time};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub async fn nat_server(addr: SocketAddr) {
    let socket = TcpSocket::new_v4().unwrap();
    socket.set_reuseaddr(true).unwrap();
    socket.set_reuseport(true).unwrap();
    socket.bind(addr).unwrap();
    let listener = socket.listen(1024).unwrap();
    info!("Listening on: {}", addr);

    loop {
        let (stream, addr) = listener.accept().await.unwrap();
        info!("Accepted connection from: {}", addr);
        tokio::spawn(async move {
            let mut stream = Framed::new(stream, LengthDelimitedCodec::new());
            stream
                .send(bytes::Bytes::from("Hello, world!"))
                .await
                .unwrap();

            while let Some(msg) = stream.next().await {
                let msg = msg.unwrap();
                info!(
                    "Received message: {:?}",
                    String::from_utf8(msg.to_vec()).unwrap()
                );
            }
        });
    }
}

pub fn create_socket() -> (TcpSocket, SocketAddr) {
    let socket = TcpSocket::new_v4().unwrap();
    socket.set_reuseaddr(true).unwrap();
    socket.set_reuseport(true).unwrap();
    socket.bind("0.0.0.0:0".parse().unwrap()).unwrap();
    let addr = socket.local_addr().unwrap();
    (socket, addr)
}

pub async fn nat_client(socket: TcpSocket, addr: SocketAddr) {
    let listen_addr = socket.local_addr().unwrap();
    let stream = socket.connect(addr).await.unwrap();
    let mut stream = Framed::new(stream, LengthDelimitedCodec::new());

    if let Some(msg) = stream.next().await {
        let msg = msg.unwrap();
        let msg = serde_json::from_slice::<HashMap<String, String>>(&msg).unwrap();
        let nat_addr: SocketAddr = msg.get("address").unwrap().parse().unwrap();
        info!("Received address: {}", nat_addr);

        let (tx, mut rx) = tokio::sync::mpsc::channel(2);

        tokio::spawn(async move {
            let stream = loop {
                let socket = TcpSocket::new_v4().unwrap();
                socket.set_reuseaddr(true).unwrap();
                socket.set_reuseport(true).unwrap();
                socket.bind(listen_addr).unwrap();

                match time::timeout(time::Duration::from_millis(200), socket.connect(nat_addr))
                    .await
                {
                    Ok(Ok(stream)) => break Ok(stream),
                    Err(err) => {
                        info!("Failed to connect to NAT: {}", err);
                        continue;
                    }
                    Ok(Err(err)) => {
                        if err.kind() == std::io::ErrorKind::AddrNotAvailable {
                            break Err(err);
                        }
                        info!("Failed to connect to NAT: {}, {}", err.kind(), err);
                        continue;
                    }
                }
            };
            tx.send(()).await.unwrap();
            if let Ok(stream) = stream {
                let remote_addr = stream.peer_addr().unwrap();
                info!("remote addr: {}", remote_addr);

                let mut stream = Framed::new(stream, LengthDelimitedCodec::new());
                stream
                    .send(bytes::Bytes::from("Hello, world!"))
                    .await
                    .unwrap();
                while let Some(msg) = stream.next().await {
                    let msg = msg.unwrap();
                    info!(
                        "Received message: {:?}, from: {}",
                        String::from_utf8(msg.to_vec()).unwrap(),
                        remote_addr
                    );
                }
            }
        });

        rx.recv().await.unwrap();

        stream
            .send(bytes::Bytes::from("NAT traversal complete!"))
            .await
            .unwrap();

        rx.recv().await.unwrap();
    }
}
