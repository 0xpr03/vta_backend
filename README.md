#  VTA Sync Backend

Sync backend for [VTA](https://github.com/0xpr03/VocableTrainer-Android)

## Building

Requirements:
- [rust](https://www.rust-lang.org/tools/install)

Release build:  
```bash
cargo build --release
```
Final files are inside `target/release`

## Running

Development:  
`cargo run`

Production:
Use the release build or run `cargo run --release`

## Configuration

Copy `config/default.toml` to `config/config.toml` and edit it.

## Development setup

The following environment variables have to be set up for vscode:
- mariadb database

Example in windows terminal:
```powershell
$env:DATABASE_URL="mysql://root@localhost/vta_sync"
& "C:\Users\<User>\AppData\Roaming\Microsoft\Windows\Start Menu\Programs\Visual Studio Code\Visual Studio Code.lnk"
```

## Debugging tests

Set the rust log via `$Env:RUST_LOG='vta_sync_backend=trace'` or `RUST_LOG=vta_sync_backend=trace` and then run the specific test.
trace-logging in tests can be enabled using `#[test_log::test(actix_rt::test)]` instead of `#[actix_rt::test]` or `tokio::test`.