# Dojo client

Introduces JS provider to interact with Dojo World (via Torii).

Loads dojo systems and executes them in locally in blockifier updating local state instantaneously. Reconciles with sequencer state when transaction is processed.

## 🚴 Usage

Install [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) for building/testing.

### Build with `wasm-pack build`

```
wasm-pack build
```

### Test in Headless Browsers with `wasm-pack test`

```
wasm-pack test --headless --firefox
```
