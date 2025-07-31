# mdbook-d2-png

A PNG-output mdBook preprocessor for D2 diagrams. Requires the `d2` CLI on PATH.

## Install

```sh
cargo install --path . --locked
```

## Configure in your book

Add this to `book.toml`:

```toml
[preprocessor.d2-png]
# path to d2 binary (optional, default: "d2")
path = "d2"

# layout engine (optional, default: "dagre")
layout = "dagre"

# PNG behavior (default false): when true, diagrams are inlined via base64 data URIs
inline = false

# output directory relative to `src/` for generated diagrams (used when inline = false)
output-dir = "d2"
```

## Use in Markdown

```md
```d2
a: A
b: B
a -> b: hello
```
```

## Thanks

The code in this preprocessor is based on that from <https://github.com/matthiasbeyer/mdbook-svgbob2>

---

*Was this useful? [Buy me a coffee](https://github.com/sponsors/danieleades/sponsorships?sponsor=danieleades&preview=true&frequency=recurring&amount=5)*
