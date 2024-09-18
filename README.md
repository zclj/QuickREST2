# QuickREST 2

QuickREST can be used to explore different behaviours of a REST API.

This is a new version of the original [QuickREST](https://github.com/zclj/QuickREST). It contains a CLI which corresponds to the original version but in addition also an experimental user interface.

## Build

Building QuickREST 2 requires a working [Rust](https://rustup.rs/) toolchain.

- Build the CLI with `sh build-cli.sh`

- Build the UI with `sh build-ui.sh`

There is also a Docker-file for building in `./build/Dockerfile`

## Development

To change log levels, use `RUST_LOG` environment variable. For example, `RUST_LOG=info cargo r --bin app`. Read more on logging configuration [here](https://docs.rs/env_logger/latest/env_logger/#enabling-logging).

## Running

After a build, the UI application executable can be found at `./target/release/app`. The CLI application can be found at `./target/release/cli`.

The UI application currently lacks documentation.

The QuickREST CLI can be used to either explore behaviours or run previously found behaviours as tests. For the available options use `cli explore --help` or `cli test --help`. In addition, the guide of the different options from the original QuickREST should be similar.
