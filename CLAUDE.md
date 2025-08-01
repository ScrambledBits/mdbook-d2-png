# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

mdbook-d2-png is a PNG-output mdBook preprocessor for D2 diagrams. It's a fork of [mdbook-d2](https://github.com/danieleades/mdbook-d2) that outputs PNG images instead of SVG. It's a Rust library and CLI tool that converts fenced `d2` code blocks in markdown files into PNG images, either as separate files or inline base64 data URIs.

### Key Fork Changes

- **Output format**: PNG instead of SVG (`.png` file extension)
- **Default inline behavior**: `inline = false` by default (separate files instead of embedded)
- **Preprocessor name**: `d2-png` instead of `d2`
- **Base64 encoding**: Uses base64 data URIs when `inline = true`
- **D2 integration**: Relies on d2's native PNG output without `--output-format` flag

## Build and Development Commands

```bash
# Build the project
cargo build

# Build for release
cargo build --release

# Run tests
cargo test

# Install locally from source
cargo install --path . --locked

# Run with example
cd example && mdbook build
```

## Architecture

### Core Components

- **`src/main.rs`**: CLI entry point using clap for argument parsing. Handles mdBook preprocessor protocol.
- **`src/lib.rs`**: Main preprocessor implementation (`D2` struct implementing `Preprocessor` trait). Processes markdown events and converts D2 code blocks.
- **`src/backend.rs`**: Contains `Backend` struct that handles D2 binary execution, file generation, and output formatting (inline vs embedded PNG).
- **`src/config.rs`**: Configuration parsing from `book.toml` with defaults and validation.

### Key Processing Flow

1. mdBook calls the preprocessor with book content
2. `process_events` function iterates through markdown events looking for `d2` code blocks
3. D2 code blocks are extracted and processed
4. `Backend.render()` calls the D2 binary to generate PNG files:
   - For `inline = false`: Creates PNG file in output directory, returns image reference
   - For `inline = true`: Creates PNG file, reads it, encodes as base64 data URI
5. Generated PNGs are either inlined as base64 or referenced as file paths in markdown events

### Configuration Options

Set in `[preprocessor.d2-png]` section of `book.toml`:
- `path`: D2 binary path (default: "d2")
- `layout`: Layout engine (default: None, d2 default is "dagre")
- `inline`: Inline PNG as base64 data URI (default: false)
- `output-dir`: Output directory under src/ (default: "d2")
- `theme`: Optional theme
- `dark-theme`: Optional dark theme
- `fonts`: Custom font configuration

## Testing

Tests are located in `tests/` directory with test books in `tests/library/`:
- `simple`: Basic file-based PNG output
- `inline`: Base64 inline PNG output
- `custom-src`: Custom source directory configuration

Test helper is in `tests/common/mod.rs` with `TestBook` struct.

## Dependencies

- `mdbook`: Integration with mdBook preprocessor system
- `pulldown-cmark`: Markdown parsing and event processing
- `clap`: Command-line argument parsing
- `serde`/`toml`: Configuration deserialization
- `base64`: For inline PNG encoding
- `anyhow`: Error handling

## External Requirements

- D2 binary must be installed and available on PATH (compatible with d2 >=0.7.0)
- **Important**: No `--output-format` flag is used; relies on d2's native PNG output capability
- The preprocessor simply passes input `.d2` content via stdin and output `.png` file path as argument