#!/bin/bash
export CARGO_TARGET_MIPS_UNKNOWN_LINUX_MUSL_LINKER=/home/justin/repos/althea-firmware/build/staging_dir/toolchain-mips_24kc_gcc-5.5.0_musl/bin/mips-openwrt-linux-gcc
export OPENSSL_DIR=/home/justin/repos/althea-firmware/build/staging_dir/host/
export STAGING_DIR=/home/justin/repos/althea-firmware/build/staging_dir/
export PKG_CONFIG_ALLOW_CROSS=1

cargo clean
cargo build --target mips-unknown-linux-musl