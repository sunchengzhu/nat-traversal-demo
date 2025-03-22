use nat_traversal_test::{
    tcp::{create_socket, nat_client, nat_server},
    tcp_stun_server,
    udp::nat_client as udp_nat_client,
    udp_stun_server,
};

use std::net::SocketAddr;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let matches = clap::Command::new("nat_traversal")
        .name("Nat traversal demo")
        .about("Nat traversal demo on TCP and UDP")
        .version(clap::crate_version!())
        .arg(
            clap::Arg::new("address")
                .help("stun server address")
                .value_parser(clap::value_parser!(SocketAddr)),
        )
        .arg(
            clap::Arg::new("protocol")
                .short('p')
                .help("use TCP or UDP")
                .value_parser(["tcp", "udp"])
                .default_value("tcp")
                .action(clap::ArgAction::Set),
        )
        .get_matches();
    let protocol = matches.get_one::<String>("protocol").unwrap();

    if std::env::args().len() == 1 {
        rt.spawn(udp_stun_server());
        rt.block_on(tcp_stun_server());
    }

    let stun_addr = *matches.get_one::<SocketAddr>("address").unwrap();
    if protocol == "tcp" {
        let stun_addr = std::env::args().nth(1).unwrap().parse().unwrap();
        let (socket, listen_addr) = create_socket();
        rt.spawn(nat_client(socket, stun_addr));
        rt.block_on(nat_server(listen_addr));
    } else if protocol == "udp" {
        rt.block_on(udp_nat_client(stun_addr));
    }
}
