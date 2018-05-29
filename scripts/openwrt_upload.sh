#!/usr/bin/env bash
set -eux
export TARGET=mipsel
export TRIPLE=mipsel-unknown-linux-musl
export ROUTER_IP=192.168.10.1
bash scripts/openwrt_build_$TARGET.sh
scp target/$TRIPLE/release/rita root@$ROUTER_IP:/tmp/rita
ssh root@$ROUTER_IP RUST_BACKTRACE=FULL RUST_LOG=TRACE /tmp/rita --config=/etc/rita.toml --platform=linux &> out.log
