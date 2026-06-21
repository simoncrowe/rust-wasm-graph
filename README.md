# rust-wasm-centrality

Interactive network graph visualization with centrality ranking, built with Rust/WebAssembly and WebGL.

Nodes are rendered on an interactive canvas (pan, zoom, pinch-to-zoom) and ranked by centrality in a searchable sidebar table. Selecting a node in the table pans the camera to it.

## Build

Compile Rust to WebAssembly:

```shell
wasm-pack build --target web
```

Then serve the project root with any static file server and open `index.html`.

## License

MIT
