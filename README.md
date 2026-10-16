# RDeskMini

<p align="center">
  <img width="256" height="256" src="./assets/logo.png" />
</p>

**RDeskMini** is a free and open remote desktop tool, written in Rust. The goal is to offer the simplicity of AnyDesk or TeamViewer without their restrictions: open the program on two computers, and one of them can see and control the other.

The target audience is people who are not technical. The interface should be simple, and the program should work by opening a single file, with no installation steps.

> **Status: early prototype (v0.1.x).** Connections are made by typing the IP address of the other computer. There is no password yet and no peer-to-peer discovery yet. Do not expose the port to the internet. See the [Roadmap](#roadmap).

## Features (current)

- **Host and client modes**: the host shares its screen, and the client controls it.
- **Screen streaming** with H.264 video over UDP, with Reed-Solomon forward error correction to tolerate packet loss.
- **Mouse and keyboard control** on the remote computer.
- **Encrypted control channel** (TLS). The certificate is created automatically on the first run, with no manual steps.
- **English and Portuguese** interface.
- **Native GUI** built with egui. No browser or WebView is needed.

## Roadmap

These items describe where the project is going. They are not implemented yet.

1. **Peer discovery**: each computer running RDeskMini shows up in a list of online peers, so users connect by choosing a name instead of typing an IP address.
2. **Peer-to-peer connections with NAT traversal**: connect computers behind home routers without port forwarding.
3. **Relay fallback**: when a direct connection is not possible, traffic goes through a relay server.
4. **Passwords and access approval**: a one-time password, or the host approves each connection.
5. **Single-file executables**: a download that runs with a double click on Linux, Windows and macOS.
6. **Simple user interface**: large buttons, plain language, and guided steps for non-technical users.
7. **Rename** the internal identifiers (`RustDesk-Mini`, `rustdesk-mini`) to `RDeskMini`.

## Requirements

- Rust 1.90 or newer (the crate uses edition 2024).
- A C/C++ compiler, because the H.264 encoder (`openh264`) is built from source.
- System libraries required by the GUI and input crates on your platform:
  - **Linux**: X11 or Wayland development packages, plus xkbcommon and related libraries. The exact package names depend on your distribution.
  - **Windows**: the MSVC build tools (or MinGW).
  - **macOS**: Xcode command line tools.

### Platform permissions

- **macOS**: the first time the host captures the screen, grant **Screen Recording** permission. To control the mouse and keyboard, grant **Accessibility** permission. Both are in *System Settings → Privacy & Security*.
- **Windows**: the first run may ask the firewall to allow the program. Allow it on private networks.
- **Linux (Wayland)**: screen capture and input injection depend on the compositor. X11 is the most reliable option today.

## Build

Build the optimized binary for your machine:

```bash
cargo build --release
```

The binary is written to `target/release/rdeskmini` (`rdeskmini.exe` on Windows).

With [`just`](https://github.com/casey/just) installed, run `just` to list all tasks. For example, `just package` builds and copies the binary to `dist/rdeskmini-<version>-<os>-<arch>`.

## Usage

Start the program on both computers.

1. On the computer you want to **control**, open RDeskMini in the **client** mode.
2. On the computer you want to **share**, keep RDeskMini in the **host** mode (the default). Note its IP address on the local network.
3. In the client, type the host address, for example `192.168.0.10:9000`, and connect.

Command-line options:

```text
rdeskmini [--mode host|client] [--lang en|pt]

  -m, --mode   host (default) or client
  -l, --lang   en (default) or pt for Portuguese
```

The default port is `9000`.

Run from source:

```bash
cargo run --release -- --mode host
cargo run --release -- --mode client --lang pt
```

## Architecture

The code is split by responsibility. Each module does one job.

| Module | Responsibility |
|---|---|
| `main.rs` | Entry point: reads the command line, prepares the certificates, and starts the GUI. |
| `app.rs` | GUI state and rendering with egui. Starts and stops the backends. |
| `host.rs` | Host side: captures the screen, streams video, and receives input events. |
| `client.rs` | Client side: connects to a host, shows the video, and sends input events. |
| `protocol.rs` | Shared constants, messages between threads and the GUI, and session types. |
| `fec.rs` | Reed-Solomon forward error correction for UDP datagrams. |
| `video.rs` | H.264 encoding and decoding, and RGB/YUV conversion. |
| `input.rs` | Applies mouse and keyboard events, and maps GUI keys to key codes. |
| `cert.rs` | Creates and loads the TLS certificate for the control channel. |
| `i18n.rs` | Translations (English and Portuguese). |

### How it works

- **Control channel**: a TLS connection over TCP. The client sends commands such as connect and input events, and the host replies.
- **Video channel**: H.264 frames split into UDP datagrams. Each group of 8 data shards has 2 parity shards, so lost packets can be rebuilt without retransmission.
- **Input**: mouse and keyboard events are sent from the client and applied on the host with `enigo`.

Certificates are stored in the per-user configuration directory (`ProjectDirs` with the `com.rustdesk-mini` identifier, see the roadmap item about renaming).

## Development

Common tasks with `just`:

| Recipe | What it does |
|---|---|
| `just run-host` / `just run-client` | Run in host or client mode |
| `just build` | Release build |
| `just package` | Release build copied to `dist/` |
| `just check` | Formatting, clippy and tests |
| `just check-all` | Everything that must pass before a release |
| `just fmt` / `just fix` | Format the code / apply clippy suggestions |
| `just audit` / `just outdated` | Security audit / outdated dependencies |
| `just clean` | Remove `target/` and `dist/` |

## Security

RDeskMini is **not ready for untrusted networks yet**:

- There is no password or approval step, so anyone who can reach the host port can request a connection.
- Only use it on networks you trust, such as your own home network, until passwords are implemented.
- The control channel is encrypted with TLS, but the certificate is self-signed and generated locally.

Please report security problems privately before opening a public issue.

## License

RDeskMini is free software, licensed under the **GNU General Public License v3.0** (see [LICENSE](LICENSE)).
