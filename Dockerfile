FROM rust:1.85.0 as build
WORKDIR /usr/src/nat-traversal
COPY . .
RUN cd /usr/src/nat-traversal && cargo build --release

FROM rust:1.85.0
RUN apt-get update && apt-get install -y sudo curl tcpdump iptables iproute2 dnsutils iputils-ping && rm -rf /var/lib/apt/lists/* && update-alternatives --set iptables /usr/sbin/iptables-legacy
RUN echo 'user ALL=(root) NOPASSWD:/usr/sbin/iptables' >> /etc/sudoers


WORKDIR /app
COPY --from=build /usr/src/nat-traversal/target/release/nat_traversal /app/nat-traversal
CMD ["tail", "-f", "/dev/null"]
