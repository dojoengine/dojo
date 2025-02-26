# Katana Explorer

This crate provides the Katana Explorer UI server functionality. The UI code is maintained in a separate repository and included here as a git submodule.

## Structure

The explorer UI code is located in the `ui` directory as a git submodule. The built files must be present in `ui/dist` directory for the explorer to work.

## Building the Explorer

### In CI

The explorer UI is automatically built in CI before building Katana. This ensures that the explorer UI is always available when building official releases.

### Building Locally

If you need to build the explorer UI locally:

1. Initialize the submodule:

```bash
git submodule update --init --recursive
```

2. Build the UI:

```bash
cd ui
bun install
bun run build
```

The build output will be placed in `ui/dist`. This directory must exist and contain the built files for the explorer to work.

## Runtime Behavior

The explorer will fail to start if the UI build files are not found. This is intentional to prevent running with missing or outdated UI files.

## Development

For local development of the UI:

1. Initialize the submodule as described above
2. Navigate to the UI directory: `cd ui`
3. Start the development server: `bun run dev`

Note: The `ui/dist` directory is gitignored to prevent committing built files.

## Utilities

This crate also provides some utility functions for working with the embedded files:

- `inject_rpc_url`: Injects the RPC URL into an HTML file
