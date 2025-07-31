use std::{io, process};

use clap::Parser;
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use mdbook_d2_png::D2;
use semver::{Version, VersionReq};

/// PNG-output mdBook preprocessor for D2 diagrams.
///
/// Converts fenced `d2` code blocks into PNG images, either referenced as files or inlined as base64 data URIs.
///
/// Configuration options (in `book.toml`):
///
/// [preprocessor.d2-png]
/// path = "d2"            # Path to d2 binary (default: "d2")
/// layout = "dagre"       # Layout engine (default: "dagre")
/// inline = false         # Inline PNG as base64 data URI (default: false)
/// output-dir = "d2"      # Output directory under src/ (default: "d2")
/// theme = "..."          # Optional theme
/// dark-theme = "..."     # Optional dark theme
///
/// Example usage:
/// ```
/// ```d2
/// a: A
/// b: B
/// a -> b: hello
/// ```
/// ```
#[derive(clap::Parser)]
#[command(
    name = "mdbook-d2-png",
    about = "PNG-output mdBook preprocessor for D2 diagrams (see [preprocessor.d2-png] in book.toml)",
    long_about = "Converts fenced d2 code blocks into PNG images for mdBook.\n\nOptions (set in book.toml):\n  path: Path to d2 binary (default: 'd2')\n  layout: Layout engine (default: 'dagre')\n  inline: Inline PNG as base64 data URI (default: false)\n  output-dir: Output directory under src/ (default: 'd2')\n  theme: Optional theme\n  dark-theme: Optional dark theme\n\nExample:\n[preprocessor.d2-png]\npath = 'd2'\nlayout = 'dagre'\ninline = false\noutput-dir = 'd2'\n"
)]
pub struct Args {
    #[clap(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Parser)]
pub enum Command {
    #[clap(
        about = "Check if a renderer is supported",
        long_about = "Checks if the given renderer is supported by this preprocessor. Used internally by mdBook."
    )]
    Supports {
        #[clap(help = "Renderer name (e.g. html)")]
        renderer: String,
    },
}

fn main() {
    let args = Args::parse();

    // Users will want to construct their own preprocessor here
    let preprocessor = D2;

    if let Some(Command::Supports { renderer }) = args.command {
        handle_supports(&preprocessor, &renderer);
    } else if let Err(e) = handle_preprocessing(&preprocessor) {
        eprintln!("{e}");
        process::exit(1);
    }
}

fn handle_preprocessing(pre: &dyn Preprocessor) -> Result<(), Error> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    let book_version = Version::parse(&ctx.mdbook_version)?;
    let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)?;

    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: The {} plugin was built against version {} of mdbook, but we're being \
             called from version {}",
            pre.name(),
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}

fn handle_supports(pre: &dyn Preprocessor, renderer: &str) -> ! {
    let supported = pre.supports_renderer(renderer);

    // Signal whether the renderer is supported by exiting with 1 or 0.
    if supported {
        process::exit(0);
    } else {
        process::exit(1);
    }
}
