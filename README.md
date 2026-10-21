# RDeskMini

<p align="center">
  <img width="256" height="256" src="./assets/logo.png" />
</p>

**RDeskMini** is a free and open remote desktop tool, written in Rust. The goal is to offer the simplicity of AnyDesk or TeamViewer without their restrictions: open the program on two computers, and one of them can see and control the other.

The target audience is people who are not technical. The interface should be simple, and the program should work by opening a single file, with no installation steps.

> **Status: early prototype (v0.1.x).** Connections use a device ID and a manually verified certificate fingerprint. The host must approve each connection. There is no account/password system or peer-to-peer discovery yet. See the [Roadmap](#roadmap).

## Features (current)

- **Host and client modes**: the host shares its screen, and the client controls it.
- **Screen streaming** with H.264 video, sent over TCP through the relay server.
- **Mouse and keyboard control** on the remote computer.
- **Encrypted and pinned TLS channels**. The certificate is created automatically; users compare its SHA-256 fingerprint out of band.
- **Host approval** for each incoming control request.
- **US English, Brazilian Portuguese, and Spanish** interface.
- **Native GUI** built with egui. No browser or WebView is needed.

## Roadmap

These items describe where the project is going. They are not implemented yet.

1. **Peer discovery**: each computer running RDeskMini shows up in a list of online peers, so users connect by choosing a name instead of typing an IP address.
2. **Peer-to-peer connections with NAT traversal**: connect computers behind home routers without port forwarding.
3. **Relay fallback**: when a direct connection is not possible, traffic goes through a relay server.
4. **Account and password options**: optional one-time passwords and managed access controls.
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

With [`just`](https://github.com/casey/just) installed, run `just` to list all tasks. For example, `just package` builds and copies the native binary to `dist/`. On a Windows machine with the MSVC Rust toolchain installed, run `just package-windows` to create `dist/rdeskmini-<version>-windows-x86_64.exe`. The Windows target is also available as `just build-windows`.

## Usage

Open RDeskMini on both computers.

1. The host shares its **ID** and **SHA-256 fingerprint** with the controller.
2. Verify the fingerprint with the host through a separate trusted channel (for example, a phone call). Do not trust a fingerprint supplied only through the same untrusted relay or chat.
3. The controller enters the ID and verified fingerprint in **Connect to another device**, then clicks **Connect**.
4. The host explicitly chooses **Allow** or **Reject**. Requests expire after 30 seconds.

Both computers connect through the relay server, so they can work from different networks. TLS protects the video and input in transit, and the controller verifies the host certificate against the fingerprint. The relay passes encrypted traffic but can observe connection metadata and can deny service.

Language: RDeskMini detects Brazilian Portuguese (`pt_BR`) or Spanish (`es`) from the system locale, and defaults to US English. You can change the language in the top-right corner.

## Relay server

The relay server pairs the two computers. It runs on any machine with a public address (for example a small VPS).

1. Build it: `just server-build`. The binary is `server/target/release/rdesk-relay`.
2. Open TCP port **9443** in the server's firewall.
3. Run it: `./rdesk-relay`. To keep it running after a reboot, create a systemd service that runs it.
4. Set `RELAY_ADDR` in `src/protocol.rs` to your server's address (for example `relay.example.com:9443`) before building RDeskMini.

The relay does not store traffic or decrypt session data. It assigns IDs but does not authenticate an ID-to-host mapping; controllers authenticate the host by comparing its certificate fingerprint through a separate trusted channel. Knowing an ID alone does not grant control: the host must approve each connection. The relay is still a prototype and does not yet provide production-grade abuse protection or availability.

## Architecture

The code is split by responsibility. Each module does one job.

| Module | Responsibility |
|---|---|
| `main.rs` | Entry point: detects the language, prepares the certificates, and starts the GUI with the host running. |
| `app.rs` | GUI state and rendering with egui. Starts and stops the backends. |
| `host.rs` | Host side: registers with the relay, then streams the screen and applies the controller's input. |
| `client.rs` | Controller side: requests a session by ID, shows the remote screen and sends input. |
| `protocol.rs` | Shared constants, messages between threads and the GUI, and session types. |
| `relay_proto.rs` | Messages between the apps and the relay server. |
| `video.rs` | H.264 encoding and decoding, and RGB/YUV conversion. |
| `input.rs` | Applies mouse and keyboard events, and maps GUI keys to key codes. |
| `identity.rs` | Formats and reads the 9-digit IDs. |
| `cert.rs` | Creates and loads the TLS certificate for the control channel. |
| `i18n.rs` | Translations (US English, Brazilian Portuguese, and Spanish). |

### How it works

- **Control channel**: a TLS connection over TCP. The client sends commands such as connect and input events, and the host replies.
- **Video channel**: each H.264 frame is sent as one length-prefixed message over TCP.
- **Input**: mouse and keyboard events are sent from the client and applied on the host with `enigo`.

Certificates are stored in the per-user configuration directory (`ProjectDirs` with the `com.rustdesk-mini` identifier, see the roadmap item about renaming).

## Development

Common tasks with `just`:

| Recipe | What it does |
|---|---|
| `just run` | Run RDeskMini |
| `just build` | Release build |
| `just package` | Release build copied to `dist/` |
| `just check` | Formatting, clippy and tests |
| `just check-all` | Everything that must pass before a release |
| `just fmt` / `just fix` | Format the code / apply clippy suggestions |
| `just audit` / `just outdated` | Security audit / outdated dependencies |
| `just clean` | Remove `target/` and `dist/` |

## Security

RDeskMini is still an early prototype. Before connecting, compare the displayed certificate fingerprint through a trusted channel:

- The host must approve every request, but there is no account or password system.
- The relay can observe metadata, block connections, or attempt impersonation if users do not verify the fingerprint independently.
- Use only relays you trust; the relay has no rate limiting or production-grade abuse controls yet.

Please report security problems privately before opening a public issue.

## License

RDeskMini is free software, licensed under the **GNU General Public License v3.0** (see [LICENSE](LICENSE)).
