# RDeskMini

<p align="center">
  <img width="256" height="256" src="./assets/logo.png" />
</p>

[![Rust CI](https://github.com/waldirborbajr/NoIPClient/actions/workflows/rust-ci.yaml/badge.svg)](https://github.com/waldirborbajr/NoIPClient/actions/workflows/rust-ci.yaml)
[![Release](https://github.com/waldirborbajr/NoIPClient/actions/workflows/release.yaml/badge.svg)](https://github.com/waldirborbajr/NoIPClient/actions/workflows/release.yaml)
[![Release to crates.io](https://github.com/waldirborbajr/NoIPClient/actions/workflows/crates.yaml/badge.svg)](https://github.com/waldirborbajr/NoIPClient/actions/workflows/crates.yaml)
[![Security Audit](https://github.com/waldirborbajr/NoIPClient/actions/workflows/security-audit.yaml/badge.svg)](https://github.com/waldirborbajr/NoIPClient/actions/workflows/security-audit.yaml)

A Rust client that keeps one or more No-IP hostnames (https://www.noip.com/) pointed at your current public IP.

It runs in one of three ways: as a systemd timer (default), as a long-lived daemon (`--daemon`), or in the background on Termux or a notebook with the included `noipctl.sh` script. It is safe against transient network failures, honors No-IP's abuse rate limit, and can notify you on Telegram on every run, or only when something changed.

## Features
