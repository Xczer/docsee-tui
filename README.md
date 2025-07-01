# DocSee

A terminal UI for managing Docker, written in Rust with [Ratatui](https://github.com/ratatui-org/ratatui).

I use Docker daily and got tired of typing `docker ps`, `docker logs -f`, `docker exec -it ... sh` a hundred times a day. Wanted something like `k9s` but for plain Docker, so I built this. It lets you browse containers, images, volumes and networks, tail logs, drop into a container shell and watch resource usage - all from the terminal.

[![CI](https://github.com/Xczer/docsee-tui/actions/workflows/ci.yml/badge.svg)](https://github.com/Xczer/docsee-tui/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

```
██████╗  ██████╗  ██████╗███████╗███████╗███████╗
██╔══██╗██╔═══██╗██╔════╝██╔════╝██╔════╝██╔════╝
██║  ██║██║   ██║██║     ███████╗█████╗  █████╗
██║  ██║██║   ██║██║     ╚════██║██╔══╝  ██╔══╝
██████╔╝╚██████╔╝╚██████╗███████║███████╗███████╗
╚═════╝  ╚═════╝  ╚═════╝╚══════╝╚══════╝╚══════╝
```

## What it does

- Containers - start / stop / restart / delete, with status colours
- Images - list, delete, prune
- Volumes and networks - list and manage
- Logs - live tailing with timestamps, word wrap and filtering
- Shell - exec into a running container without leaving the TUI
- Stats - live CPU / memory / network / disk per container
- Search - `/` to filter anything on screen

Works on Linux, macOS and Windows.

## Build

Needs a recent stable Rust toolchain and Docker running locally.

```bash
git clone https://github.com/Xczer/docsee-tui.git
cd docsee-tui
cargo build --release
# binary lands in target/release/docsee
```

Or install straight from the source tree:

```bash
cargo install --path .
```

## Usage

```bash
# default docker socket
docsee

# remote docker over tcp
docsee --docker-host tcp://remote-host:2375

# over ssh
docsee --docker-host ssh://user@remote-host
```

It also picks up the usual `DOCKER_HOST` / `DOCKER_TLS_VERIFY` / `DOCKER_CERT_PATH` env vars if they are set.

## Keys

Global

| Key | Action |
|-----|--------|
| `←/→` | switch tabs |
| `↑/↓` | move in list |
| `Enter` | select |
| `Esc` | go back |
| `q` | quit |
| `c` | help / cheatsheet |

Containers

| Key | Action |
|-----|--------|
| `u` | start |
| `d` | stop |
| `r` | restart |
| `D` | delete |
| `l` | logs |
| `e` | shell |
| `s` | stats |
| `/` | search |

Logs viewer

| Key | Action |
|-----|--------|
| `f` | follow on/off |
| `t` | timestamps |
| `w` | word wrap |
| `c` | clear |
| `PgUp/PgDn` | page |

Full list is inside the app, just hit `c`.

## Layout

```
src/
├── app.rs        # main loop, tab routing, key dispatch
├── docker/       # bollard wrapper - containers, images, volumes, networks, system
├── events/       # keyboard / timer event handling
├── ui/           # per-tab views + logs/shell/stats viewers
└── widgets/      # small reusable bits (tables, modals)
```

Async everywhere via Tokio so the UI never blocks on a Docker call.

## Dev

```bash
cargo run                # run against local docker
cargo fmt
cargo clippy
cargo test
```

## Troubleshooting

Connection error - make sure the daemon is up (`docker info`) and you can read the socket (`ls -la /var/run/docker.sock`).

Colours look off - check your terminal, `TERM=xterm-256color docsee` usually fixes it. Needs at least an 80x24 terminal.

## Thanks

Built on top of [Ratatui](https://github.com/ratatui-org/ratatui), [Bollard](https://github.com/fussybeaver/bollard) and [Tokio](https://tokio.rs/). Design ideas borrowed shamelessly from [k9s](https://github.com/derailed/k9s).

## License

MIT - see [LICENSE](LICENSE).
