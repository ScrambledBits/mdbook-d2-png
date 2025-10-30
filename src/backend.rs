use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context};
use mdbook::book::SectionNumber;
use mdbook::preprocess::PreprocessorContext;
use pulldown_cmark::{CowStr, Event, LinkType, Tag, TagEnd};

use crate::config::{Config, Fonts};

/// Configuration key in book.toml for this preprocessor
const PREPROCESSOR_CONFIG_KEY: &str = "preprocessor.d2-png";

/// Represents the backend for processing D2 diagrams
pub struct Backend {
    /// Absolute path to the D2 binary
    path: PathBuf,
    /// Relative path to the output directory for generated diagrams
    output_dir: PathBuf,
    /// Absolute path to the source directory of the book
    source_dir: PathBuf,
    /// Layout engine to use for D2 diagrams
    layout: Option<String>,
    inline: bool,
    fonts: Option<Fonts>,
    theme_id: Option<String>,
    dark_theme_id: Option<String>,
}

/// Context for rendering a specific diagram
#[derive(Debug, Clone, Copy)]
pub struct RenderContext<'a> {
    /// Relative path to the current chapter file
    path: &'a Path,
    /// Name of the current chapter
    chapter: &'a str,
    /// Section number of the current chapter
    section: Option<&'a SectionNumber>,
    /// Index of the current diagram within the chapter
    diagram_index: usize,
}

impl<'a> RenderContext<'a> {
    /// Creates a new [`RenderContext`]
    pub const fn new(
        path: &'a Path,
        chapter: &'a str,
        section: Option<&'a SectionNumber>,
        diagram_index: usize,
    ) -> Self {
        Self {
            path,
            chapter,
            section,
            diagram_index,
        }
    }
}

/// Generates a filename for a diagram based on its context
///
/// Returns a relative path for the diagram file
fn filename(ctx: &RenderContext) -> String {
    format!(
        "{}{}.png",
        ctx.section.cloned().unwrap_or_default(),
        ctx.diagram_index
    )
}

/// Creates markdown events for an image
///
/// Wraps an image in a paragraph with the given URL
///
/// # Arguments
/// * `url` - The image URL (can be a file path or data URI)
fn create_image_events(url: String) -> Vec<Event<'static>> {
    vec![
        Event::Start(Tag::Paragraph),
        Event::Start(Tag::Image {
            link_type: LinkType::Inline,
            dest_url: url.into(),
            title: CowStr::Borrowed(""),
            id: CowStr::Borrowed(""),
        }),
        Event::End(TagEnd::Image),
        Event::End(TagEnd::Paragraph),
    ]
}

impl Backend {
    /// Creates a new Backend instance
    ///
    /// # Arguments
    /// * `config` - Configuration for the D2 preprocessor
    /// * `source_dir` - Absolute path to the book's source directory
    pub fn new(config: Config, source_dir: PathBuf) -> Self {
        Self {
            path: config.path,
            output_dir: config.output_dir,
            layout: config.layout,
            inline: config.inline,
            source_dir,
            fonts: config.fonts,
            theme_id: config.theme_id,
            dark_theme_id: config.dark_theme_id,
        }
    }

    /// Creates a Backend instance from a [`PreprocessorContext`]
    ///
    /// # Arguments
    /// * `ctx` - The preprocessor context
    ///
    /// # Panics
    /// Panics if the d2-png preprocessor configuration is missing or invalid in book.toml
    pub fn from_context(ctx: &PreprocessorContext) -> Self {
        let config: Config = ctx
            .config
            .get_deserialized_opt(PREPROCESSOR_CONFIG_KEY)
            .unwrap_or_else(|e| {
                panic!("Unable to deserialize d2-png preprocessor config: {}", e)
            })
            .unwrap_or_else(|| {
                panic!(
                    "d2-png preprocessor config not found. Add [{}] section to book.toml",
                    PREPROCESSOR_CONFIG_KEY
                )
            });
        let source_dir = ctx.root.join(&ctx.config.book.src);

        Self::new(config, source_dir)
    }

    /// Returns the relative path to the output directory
    fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    /// Constructs the absolute file path for a diagram
    ///
    /// # Arguments
    /// * `ctx` - The render context for the diagram
    fn filepath(&self, ctx: &RenderContext) -> PathBuf {
        let filepath = Path::new(&self.source_dir).join(self.relative_file_path(ctx));
        filepath
    }

    /// Constructs the relative file path for a diagram
    ///
    /// # Arguments
    /// * `ctx` - The render context for the diagram
    fn relative_file_path(&self, ctx: &RenderContext) -> PathBuf {
        let filename = filename(ctx);
        self.output_dir.join(filename)
    }

    /// Renders a D2 diagram and returns the appropriate markdown events
    ///
    /// # Arguments
    /// * `ctx` - The render context for the diagram
    /// * `content` - The D2 diagram content
    pub fn render(
        &self,
        ctx: &RenderContext,
        content: &str,
    ) -> anyhow::Result<Vec<Event<'static>>> {
        if self.inline {
            self.render_inline_png(ctx, content)
        } else {
            self.render_embedded_png(ctx, content)
        }
    }

    /// Generates a D2 diagram PNG file
    ///
    /// Creates the output directory if needed, builds command arguments,
    /// and executes the D2 process to generate the PNG file.
    ///
    /// # Arguments
    /// * `ctx` - The render context for the diagram
    /// * `content` - The D2 diagram content
    ///
    /// # Returns
    /// The absolute path to the generated PNG file
    fn generate_diagram(
        &self,
        ctx: &RenderContext,
        content: &str,
    ) -> anyhow::Result<PathBuf> {
        use std::fs;

        // Ensure output directory exists
        let output_path = Path::new(&self.source_dir).join(self.output_dir());
        fs::create_dir_all(&output_path)
            .with_context(|| format!("Failed to create output directory: {:?}", output_path))?;

        // Build command arguments and execute D2
        let mut args = self.basic_args();
        let filepath = self.filepath(ctx);
        args.push(filepath.as_os_str());

        self.run_process(ctx, content, args)?;

        Ok(filepath)
    }

    fn render_inline_png(
        &self,
        ctx: &RenderContext,
        content: &str,
    ) -> anyhow::Result<Vec<Event<'static>>> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;
        use std::fs;

        let filepath = self.generate_diagram(ctx, content)?;
        let bytes = fs::read(&filepath)
            .with_context(|| format!("Failed to read generated PNG file: {:?}", filepath))?;
        let data_uri = format!("data:image/png;base64,{}", STANDARD.encode(bytes));
        Ok(create_image_events(data_uri))
    }

    fn render_embedded_png(
        &self,
        ctx: &RenderContext,
        content: &str,
    ) -> anyhow::Result<Vec<Event<'static>>> {
        self.generate_diagram(ctx, content)?;

        let rel_path = self.calculate_relative_path_for_chapter(ctx);
        let url = rel_path
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");

        Ok(create_image_events(url))
    }

    /// Calculates the relative path from a chapter to its diagram file
    ///
    /// This determines how many directories deep the chapter is from the source root,
    /// then builds a path with the appropriate number of "../" segments to reach
    /// the diagram file in the output directory.
    ///
    /// # Arguments
    /// * `ctx` - The render context for the diagram
    ///
    /// # Returns
    /// A relative path from the chapter's location to the diagram file
    fn calculate_relative_path_for_chapter(&self, ctx: &RenderContext) -> PathBuf {
        // Count how many directory levels the chapter is nested from the source root
        // We need to go up this many levels to reach the source root, then down into the output dir
        let parent_path = ctx.path.parent().unwrap_or_else(|| Path::new(""));
        let depth = parent_path.components().count();

        // Build path with "../" for each level we need to ascend
        let mut rel_path = PathBuf::new();
        for _ in 0..depth {
            rel_path.push("..");
        }

        // Add the path to the diagram file
        rel_path.join(self.relative_file_path(ctx))
    }

    fn basic_args(&self) -> Vec<&OsStr> {
        let mut args = vec![];

        if let Some(fonts) = &self.fonts {
            args.extend([
                OsStr::new("--font-regular"),
                fonts.regular.as_os_str(),
                OsStr::new("--font-italic"),
                fonts.italic.as_os_str(),
                OsStr::new("--font-bold"),
                fonts.bold.as_os_str(),
            ]);
        }
        if let Some(layout) = &self.layout {
            args.extend([OsStr::new("--layout"), layout.as_ref()]);
        }
        if let Some(theme_id) = &self.theme_id {
            args.extend([OsStr::new("--theme"), theme_id.as_ref()]);
        }
        if let Some(dark_theme_id) = &self.dark_theme_id {
            args.extend([OsStr::new("--dark-theme"), dark_theme_id.as_ref()]);
        }
        args.push(OsStr::new("-"));
        args
    }

    /// Runs the D2 process to generate a diagram
    ///
    /// # Arguments
    /// * `ctx` - The render context for the diagram
    /// * `content` - The D2 diagram content
    /// * `args` - Additional arguments for the D2 process
    fn run_process<I, S>(
        &self,
        ctx: &RenderContext,
        content: &str,
        args: I,
    ) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut child = Command::new(&self.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(args)
            .spawn()
            .context("Failed to spawn D2 process")?;

        // Write to stdin safely
        {
            let stdin = child.stdin.as_mut()
                .context("Failed to open stdin for D2 process")?;
            stdin.write_all(content.as_bytes())
                .context("Failed to write D2 diagram content to stdin")?;
        }

        let output = child.wait_with_output()
            .context("Failed to wait for D2 process to complete")?;

        if output.status.success() {
            Ok(())
        } else {
            let src =
                format!("\n{}", String::from_utf8_lossy(&output.stderr)).replace('\n', "\n  ");
            let msg = format!(
                "failed to compile D2 diagram ({}, #{}):{src}",
                ctx.chapter, ctx.diagram_index
            );
            bail!(msg)
        }
    }
}
