# mdbook-d2-png

A PNG-output mdBook preprocessor for D2 diagrams. Requires the `d2` CLI on PATH (compatible con d2 >=0.7.0).

## Install

```sh
cargo install --path . --locked
```

## Configure in your book

Agrega esto a tu `book.toml`:

```toml
[preprocessor.d2-png]
# path to d2 binary (opcional, default: "d2")
path = "d2"

# layout engine (opcional, default: "dagre")
layout = "dagre"

# PNG behavior (default false): cuando es true, los diagramas se incrustan como data URI base64
inline = false

# output directory relativo a `src/` para los diagramas generados (usado cuando inline = false)
output-dir = "d2"
```

## Uso en Markdown

```md
```d2
a: A
b: B
a -> b: hello
```
```

## Notas de compatibilidad

- No se usa el flag `--output-format`. El preprocesador solo pasa el archivo de entrada `.d2` y el archivo de salida `.png` al comando `d2`.
- Compatible con d2 >=0.7.0.
- Si tienes una versión anterior de d2, actualízala para soporte PNG directo.

## Thanks

The code in this preprocessor is based on that from <https://github.com/matthiasbeyer/mdbook-svgbob2>

---

*Was this useful? [Buy me a coffee](https://github.com/sponsors/danieleades/sponsorships?sponsor=danieleades&preview=true&frequency=recurring&amount=5)*
