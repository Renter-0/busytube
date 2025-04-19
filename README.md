# About

Simple CLI tool to extract video's title, duration and thumbnail. Created to work with my specific setup for Obsidian's [`Dataview`](https://blacksmithgu.github.io/obsidian-dataview/) plugin and as a project to learn stuff. It's a work in progress with some ruff edges.

# Installation

## Via Nix Flakes

Download inside a shell
```bash
nix shell github:Renter-0/busytube
```

## Compiling from Source

### Prerequisites

- `cargo` to compile the project. Can be installed via [`rustup`](https://www.rust-lang.org/tools/install)
- C compiler (`gcc`)
- `git` to clone the repository (Optional)

### Compiling

```bash
# Download the repository from GitHub
git clone https://github.com/Renter-0/busytube.git
cd busytube
cargo build --release
```
Then the binary will be in `target/release`

# Usage

```bash
busytube --help
YouTube scrapper to get video's title, duration and thumbnail

Usage: busytube <URL_FILE> <OUTPUT_DIR>

Arguments:
  <URL_FILE>
  <OUTPUT_DIR>

Options:
  -h, --help     Print help
  -V, --version  Print version
```
