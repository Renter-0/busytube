# About

Simple CLI tool to extract video's title, duration and thumbnail. Created to work with my specific setup for Obsidian's [`Dataview`](https://blacksmithgu.github.io/obsidian-dataview/) plugin and as a project to learn stuff. It's a work in progress with some ruff edges.

# How to Use

## Prerequisites

- `cargo` to compile the project. Can be installed via [`rustup`](https://www.rust-lang.org/tools/install)
- C compiler (`gcc`)
- `git` to clone the repository (Optional)

> [!NOTE]
> If you have [`nix`](https://github.com/NixOS/nix) package manager installed with flakes enabled you can run `nix develop` to get the mandatory requisites

## Compiling

```bash
# Download the repository from GitHub
git clone https://github.com/Renter-0/busytube.git
cd busytube
cargo build --release
```

## Usage

Then the binary will be in `target/release`
To use it
```bash
# Run inside the directory where you compiled the project
./target/release/busytube FILE_WITH_URLS OUTPUT_DIR
```
