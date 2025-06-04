# QuickREST 2

QuickREST can be used to explore different behaviours of a REST API.

This is a new version of the original [QuickREST](https://github.com/zclj/QuickREST). It contains a CLI which corresponds to the original version but in addition also an experimental user interface. This is a proof-of-concept project, do not expect a production grade experience!

There's a demo available [here](./demos/QuickREST_Demo.mov). The demo show an exploration of a REST API. The API service is running in a background VM, we then import an OpenAPI file describing the available operations. Then we select a behaviour to explore. QuickREST will use this information to try and find sequences that conforms to the selected behaviour. In the demo, such an example is found. In addition, we show the automatically generated invocations and sequences executed where we can see an example of shrinking in the sequence.

## Research

The following papers give more details into the design and usage of the ideas in QuickREST:

- [Exploring API behaviours through generated examples](https://link.springer.com/article/10.1007/s11219-024-09668-2)

- [Exploring behaviours of RESTful APIs in an industrial setting](https://link.springer.com/article/10.1007/s11219-024-09686-0)

- [QuickREST: Property-based Test Generation of OpenAPI-Described RESTful APIs](https://arxiv.org/pdf/1912.09686)

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
