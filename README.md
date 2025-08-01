# mdbook-d2-png

A PNG-output mdBook preprocessor for D2 diagrams. This is a fork of [mdbook-d2](https://github.com/danieleades/mdbook-d2) that outputs PNG images instead of SVG.

Requires the `d2` CLI on PATH (compatible with d2 >=0.7.0).

## Why PNG instead of SVG?

- **Better compatibility**: PNG images work consistently across all platforms and browsers
- **Reduced HTML size**: When `inline = false` (default), images are stored as separate files instead of embedded SVG
- **Simplified rendering**: Direct PNG output from D2 without additional processing

## Installation

```sh
cargo install --path . --locked
```

## Configuration

Add this to your `book.toml`:

```toml
[preprocessor.d2-png]
# Path to d2 binary (optional, default: "d2")
path = "d2"

# Layout engine (optional, default: "dagre")
layout = "dagre"

# PNG behavior (default: false)
# When true: diagrams are embedded as base64 data URIs
# When false: diagrams are saved as separate PNG files
inline = false

# Output directory relative to `src/` for generated diagrams (used when inline = false)
output-dir = "d2"

# Optional theme configuration
# theme = "..."
# dark-theme = "..."
```

## Usage in Markdown

```md
```d2
a: A
b: B
a -> b: hello
```
```

The code block will be replaced with a PNG image in the rendered document.

## Compatibility Notes

- **D2 version**: Compatible with d2 >=0.7.0
- **No `--output-format` flag**: The preprocessor relies on d2's native PNG output by simply passing the input `.d2` file and output `.png` file to the `d2` command
- **Upgrade notice**: If you have an older version of d2, please update it for direct PNG support

## Differences from mdbook-d2

This fork makes the following key changes:

1. **Output format**: PNG images instead of SVG
2. **Default inline behavior**: `inline = false` by default (vs `true` in original)
3. **Preprocessor name**: `[preprocessor.d2-png]` instead of `[preprocessor.d2]`
4. **File extensions**: Generated files use `.png` extension
5. **Base64 encoding**: When `inline = true`, uses base64 data URIs for PNG images

## Thanks

The code in this preprocessor is based on that from <https://github.com/matthiasbeyer/mdbook-svgbob2>

---

*Was this useful? [Buy me a coffee](https://github.com/sponsors/danieleades/sponsorships?sponsor=danieleades&preview=true&frequency=recurring&amount=5)*
