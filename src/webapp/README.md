# Web App

This crate also provides a web application module.

build for development:

```bash
wasm-pack build --weak-refs --dev --features webapp
```

build for production:

```bash
wasm-pack build --weak-refs --features webapp
```