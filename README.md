# 🖥️ sysmon

A fast, lightweight system monitor for the terminal — built in Rust using `ratatui`.

![Rust](https://img.shields.io/badge/Rust-1.75+-orange?logo=rust)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS-blue)
![License](https://img.shields.io/badge/license-MIT-green)

## Features

- **Overview tab** — CPU & RAM gauges with 60-second sparkline history, per-core bar chart, kernel info, load average
- **Processes tab** — live process list sorted by CPU usage, color-coded (green → yellow → red), scrollable
- **Disk & Net tab** — per-partition usage with ASCII bars, real-time download/upload sparklines in KB/s
- Updates every second with minimal CPU overhead

## Screenshots

```
┌ SYSMON ──────────────────────────────────────────────────────────────────┐
│  hostname  |  Ubuntu 24.04  |  up 03h 22m 11s  |  18:45:02              │
└──────────────────────────────────────────────────────────────────────────┘
│  Overview  │   Processes   │   Disk & Net   │
```

## Installation

### Requirements

- Rust 1.75 or newer
- Linux or macOS (Windows via WSL)

### Build from source

```bash
git clone https://github.com/unanonimov2/sysmon
cd sysmon
cargo run --release
```

> **Note:** If your system has an older Rust version, pin these dependencies first:
> ```bash
> cargo update rayon --precise 1.10.0
> cargo update rayon-core --precise 1.12.1
> cargo update unicode-segmentation --precise 1.12.0
> ```

### Install globally

```bash
sudo cp target/release/sysmon /usr/local/bin/
sysmon
```

## Usage

| Key | Action |
|-----|--------|
| `Tab` / `→` | Next tab |
| `Shift+Tab` / `←` | Previous tab |
| `j` / `↓` | Scroll down (Processes) |
| `k` / `↑` | Scroll up (Processes) |
| `q` / `Ctrl+C` | Quit |

## Dependencies

| Crate | Purpose |
|-------|---------|
| [`ratatui`](https://github.com/ratatui-org/ratatui) | Terminal UI framework |
| [`crossterm`](https://github.com/crossterm-rs/crossterm) | Cross-platform terminal input/output |
| [`sysinfo`](https://github.com/GuillaumeGomez/sysinfo) | System information (CPU, RAM, processes, disks) |
| [`chrono`](https://github.com/chronotope/chrono) | Date and time |

## License

MIT
