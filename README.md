# About

Simple CLI tool to extract video's title, duration and thumbnail. Created to work with my specific setup for Obsidian's [`Dataview`](https://blacksmithgu.github.io/obsidian-dataview/) plugin and as a project to learn stuff. It's a work in progress with some ruff edges.

## 🌐 Network Efficiency

Busytube demonstrates an approximate **40% reduction in network usage** compared to standard access methods.

### 🧪 Test Setup

To simulate traditional access, `curl` was used to download 6 video URLs and 6 thumbnail URLs listed in the `url.md` file:

```bash
for url in $(cat url.md); do curl $url -O; done
```

> **Note**  
> Thumbnail URLs were excluded during the application's run. Instead, thumbnail links were dynamically extracted from the downloaded HTML content.

### 📊 Results

Network usage was measured using the `nethogs -v 3` command.

#### **curl (Baseline):**
![curl](https://github.com/user-attachments/assets/81cd0416-cab4-46cc-9692-9b5d41de7970)

#### **busytube (Optimized):**
![busytube](https://github.com/user-attachments/assets/a6bdaf4e-aa3d-4889-9ae9-026f8f1300f9)

### 📉 Efficiency Summary

|       Method      |Total Data Send|Total Data Recieved|
|-------------------|---------------|-------------------|
|   curl(Baseline)  |0.189 MB       |6.875 MB           |
|busytube(Optimized)|0.139 MB       |4.280 MB           |

- **Data Saved**: `6.875 MB - 4.280 MB = 2.595 MB`
- **Reduction in Network Usage**:  
$$\left( \frac{2.595}{6.875} \right) \times 100 \approx \mathbf{37.7\%}$$


# Installation

Check [releases](https://github.com/Renter-0/busytube/releases/tag/Latest) to grab version for your OS/architecture.

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
