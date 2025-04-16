# About
Simple CLI tool to extract video's title, duration and thumbnail. Created to work with my specific setup for Obsidian's dataview plugin and as a project to learn stuff.
# How to Use
## Prerequisites
For running the tool you'll need `cargo` to compile and optionally `git` to download the source code on your system. Cargo caan be installed via [`rustup`](https://www.rust-lang.org/tools/install)
```bash
git clone https://github.com/Renter-0/busytube.git
cd busytube
cargo run --release
```
Or if you're using [`nix`](https://github.com/NixOS/nix) with flakes enabled you can just run
```bash
git clone https://github.com/Renter-0/busytube.git
cd busytube
nix develop
cargo run --release
```
Then the binary will be in `target/release`
To use it
```bash
./target/release/busytube FILE_WITH_URLS OUTPUT_DIR
```
