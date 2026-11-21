# nie-model-serve

The IEVR model asset service.

- Default build: the offline catalog (`src/catalog.rs`) and the route table (`src/routes.rs`), without host facilities.
- `--features host`: the local HTTP service with GLB assembly from the live VFS and texture, audio and video routes.

Server-side rendering uses the shared CPU compositor; there is no GPU presentation. `LICENSE.upstream` preserves the source license.

Build the service with `cargo build -p nie-model-serve --features host`; run `nie-model-serve --help` for its host CLI options.
