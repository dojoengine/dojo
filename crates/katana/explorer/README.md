# Katana Explorer

This crate embeds the Katana Explorer UI files directly into the binary, eliminating the need for users to build the explorer themselves.

## Structure

The explorer build files are expected to be in the `dist` directory within this crate. These files are embedded at compile time using the `rust-embed` crate.

## Utilities

This crate also provides some utility functions for working with the embedded files:

- `inject_rpc_url`: Injects the RPC URL into an HTML file
