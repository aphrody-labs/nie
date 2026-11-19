# Build environment of linux-nie-rc: the Arch Linux userland of prd (same gcc, binutils, rust and mkinitcpio family),
# so the kernel and its modules are built by the toolchain that builds the stock `linux` package.
# `cargo xtask kernel build` builds this image once and runs every package build from it with the network off.
FROM archlinux:latest
RUN pacman -Syu --noconfirm --needed \
      base-devel bc cpio git libelf pahole perl python tar xz zstd kmod inetutils xmlto \
      rust rust-src rust-bindgen clang llvm lld \
  && pacman -Scc --noconfirm \
  && useradd -m -u 1000 builder \
  && install -d -o builder -g builder /build /out
USER builder
WORKDIR /build
