FROM rust:1.85.0 as build
WORKDIR /usr/src/nat-traversal
COPY . .
RUN cargo build --release

FROM registry.cn-hangzhou.aliyuncs.com/scz996/ckb:0.202.0-ipv4 as ckb-image

FROM rust:1.85.0
RUN apt-get update && apt-get install -y sudo curl tcpdump iptables iproute2 dnsutils iputils-ping && rm -rf /var/lib/apt/lists/* && update-alternatives --set iptables /usr/sbin/iptables-legacy
RUN echo 'user ALL=(root) NOPASSWD:/usr/sbin/iptables' >> /etc/sudoers


WORKDIR /app
COPY --from=build /usr/src/nat-traversal/target/release/nat_traversal /app/nat-traversal

RUN wget https://github.com/nervosnetwork/ckb-cli/releases/download/v1.15.0/ckb-cli_v1.15.0_x86_64-unknown-centos-gnu.tar.gz && \
    tar xvf ckb-cli_v1.15.0_x86_64-unknown-centos-gnu.tar.gz && \
    mv ckb-cli_v1.15.0_x86_64-unknown-centos-gnu/ckb-cli /app && \
    rm -rf ckb-cli_v1.15.0_x86_64-unknown-centos-gnu*

COPY --from=ckb-image /bin/ckb /bin/ckb
COPY --from=ckb-image /bin/docker-entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
CMD ["run"]
