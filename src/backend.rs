use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use mdbook::book::SectionNumber;
use mdbook::preprocess::PreprocessorContext;
use pulldown_cmark::{CowStr, Event, LinkType, Tag, TagEnd};
use smallvec::{smallvec, SmallVec};
use wait_timeout::ChildExt;

use crate::config::{Config, Fonts};

/// Configuration key in book.toml for this preprocessor
const PREPROCESSOR_CONFIG_KEY: &str = "preprocessor.d2-png";

/// Default timeout for D2 process execution
///
/// This is a reasonable timeout for most diagrams. Very complex diagrams
/// may take longer, but this helps prevent hanging on malformed input.
/// If you experience timeout issues, consider simplifying the diagram
/// or reporting a bug to the D2 project.
const D2_PROCESS_TIMEOUT: Duration = Duration::from_secs(30);

/// Path-related configuration for the backend
///
/// This struct groups all path-related fields for better organization.
/// Keeping paths separate from rendering config makes the purpose of each
/// field clearer and makes it easier to extend path configuration in the future.
#[derive(Debug, Clone)]
struct PathConfig {
    /// Absolute path to the D2 binary
    d2_binary: PathBuf,
    /// Relative path to the output directory for generated diagrams
    output_dir: PathBuf,
    /// Absolute path to the source directory of the book
    source_dir: PathBuf,
}

/// Rendering configuration for D2 diagrams
///
/// This struct groups all rendering-related options for better organization.
/// Keeping rendering config separate from paths makes the Backend structure
/// more maintainable and makes it clearer which fields affect diagram rendering.
#[derive(Debug, Clone)]
struct RenderConfig {
    /// Layout engine to use for D2 diagrams
    layout: Option<String>,
    /// Whether to inline PNG images as base64 data URIs
    inline: bool,
    /// Custom font configuration
    fonts: Option<Fonts>,
    /// Theme ID for D2 diagrams
    theme_id: Option<String>,
    /// Dark theme ID for D2 diagrams
    dark_theme_id: Option<String>,
}

/// Represents the backend for processing D2 diagrams
pub struct Backend {
    paths: PathConfig,
    render: RenderConfig,
}

/// Context for rendering a specific diagram within a chapter
///
/// This structure holds all the information needed to:
/// 1. Generate a unique filename for the diagram
/// 2. Calculate relative paths for image links
/// 3. Produce helpful error messages
#[derive(Debug, Clone, Copy)]
pub struct RenderContext<'a> {
    /// Path to the chapter file (used to calculate relative paths from chapter to diagram)
    path: &'a Path,

    /// Name of the chapter (used in error messages to identify which chapter failed)
    chapter: &'a str,

    /// Section number of the chapter (combined with `diagram_index` to create unique filenames)
    /// Example: Section "1.2" with `diagram_index` 3 becomes "1.2.3.png"
    section: Option<&'a SectionNumber>,

    /// Index of this diagram within the chapter (1-based, incremented for each diagram)
    /// Combined with section number to create unique filenames
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

/// Generates a unique filename for a diagram based on its context
///
/// Creates filenames in the format:
/// - With section: `{section}.{diagram_index}.png` (e.g., `1.2.3.png`)
/// - Without section: `{path_hash}_{diagram_index}.png` (e.g., `a1b2c3d4_1.png`)
///
/// The path hash ensures uniqueness for unnumbered chapters, preventing
/// filename collisions when multiple chapters lack section numbers.
///
/// # Arguments
/// * `ctx` - The render context containing section, path, and diagram index
fn filename(ctx: &RenderContext) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    ctx.section.as_ref().map_or_else(
        || {
            // Generate a stable hash from the chapter path for uniqueness
            let mut hasher = DefaultHasher::new();
            ctx.path.hash(&mut hasher);
            let path_hash: String = format!("{:x}", hasher.finish())
                .chars()
                .take(8)
                .collect();
            format!("{}_{}.png", path_hash, ctx.diagram_index)
        },
        // Note: SectionNumber's Display impl already includes a trailing dot (e.g., "1.2.")
        // so we just append the diagram_index and extension
        |section| format!("{}{}.png", section, ctx.diagram_index),
    )
}

/// Creates markdown events for an image
///
/// Wraps an image in a paragraph with the given URL.
/// Returns a `SmallVec` since image events are always exactly 4 elements.
///
/// # Arguments
/// * `url` - The image URL (can be a file path or data URI)
fn create_image_events(url: String) -> SmallVec<[Event<'static>; 4]> {
    smallvec![
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
        let paths = PathConfig {
            d2_binary: config.path,
            output_dir: config.output_dir,
            source_dir,
        };

        let render = RenderConfig {
            layout: config.layout,
            inline: config.inline,
            fonts: config.fonts,
            theme_id: config.theme_id,
            dark_theme_id: config.dark_theme_id,
        };

        Self { paths, render }
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
                panic!("Unable to deserialize d2-png preprocessor config: {e}")
            })
            .unwrap_or_else(|| {
                panic!(
                    "d2-png preprocessor config not found. Add [{PREPROCESSOR_CONFIG_KEY}] section to book.toml"
                )
            });
        let source_dir = ctx.root.join(&ctx.config.book.src);

        Self::new(config, source_dir)
    }

    /// Returns the relative path to the output directory
    fn output_dir(&self) -> &Path {
        &self.paths.output_dir
    }

    /// Constructs the absolute file path for a diagram
    ///
    /// # Arguments
    /// * `ctx` - The render context for the diagram
    fn filepath(&self, ctx: &RenderContext) -> PathBuf {
        self.paths.source_dir.join(self.relative_file_path(ctx))
    }

    /// Constructs the relative file path for a diagram (relative to source dir)
    ///
    /// # Arguments
    /// * `ctx` - The render context for the diagram
    fn relative_file_path(&self, ctx: &RenderContext) -> PathBuf {
        self.paths.output_dir.join(filename(ctx))
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
        if self.render.inline {
            self.render_inline_png(ctx, content).map(SmallVec::into_vec)
        } else {
            self.render_embedded_png(ctx, content).map(SmallVec::into_vec)
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
        let output_path = self.paths.source_dir.join(self.output_dir());
        fs::create_dir_all(&output_path)
            .with_context(|| format!("Failed to create output directory: {}", output_path.display()))?;

        // Build command arguments and execute D2
        let mut args = self.basic_args();
        let filepath = self.filepath(ctx);
        args.push(filepath.as_os_str());

        // When writing to file, D2 outputs nothing to stdout
        let _ = self.run_process(ctx, content, args)?;

        Ok(filepath)
    }

    fn render_inline_png(
        &self,
        ctx: &RenderContext,
        content: &str,
    ) -> anyhow::Result<SmallVec<[Event<'static>; 4]>> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        // For inline mode, don't specify an output file - D2 will output PNG to stdout
        let args = self.basic_args();
        let png_bytes = self.run_process(ctx, content, args)?;

        let data_uri = format!("data:image/png;base64,{}", STANDARD.encode(&png_bytes));
        Ok(create_image_events(data_uri))
    }

    fn render_embedded_png(
        &self,
        ctx: &RenderContext,
        content: &str,
    ) -> anyhow::Result<SmallVec<[Event<'static>; 4]>> {
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
    /// Uses pathdiff for robust cross-platform path calculation.
    ///
    /// # Arguments
    /// * `ctx` - The render context for the diagram
    ///
    /// # Returns
    /// A relative path from the chapter's location to the diagram file
    fn calculate_relative_path_for_chapter(&self, ctx: &RenderContext) -> PathBuf {
        let chapter_dir = ctx.path.parent().unwrap_or_else(|| Path::new(""));
        let diagram_path = self.relative_file_path(ctx);

        // Use pathdiff for robust relative path calculation
        // Falls back to the diagram path if diff_paths returns None (e.g., Windows cross-drive)
        pathdiff::diff_paths(&diagram_path, chapter_dir).unwrap_or(diagram_path)
    }

    fn basic_args(&self) -> Vec<&OsStr> {
        let mut args = vec![];

        if let Some(fonts) = &self.render.fonts {
            args.extend([
                OsStr::new("--font-regular"),
                fonts.regular.as_os_str(),
                OsStr::new("--font-italic"),
                fonts.italic.as_os_str(),
                OsStr::new("--font-bold"),
                fonts.bold.as_os_str(),
            ]);
        }
        if let Some(layout) = &self.render.layout {
            args.extend([OsStr::new("--layout"), layout.as_ref()]);
        }
        if let Some(theme_id) = &self.render.theme_id {
            args.extend([OsStr::new("--theme"), theme_id.as_ref()]);
        }
        if let Some(dark_theme_id) = &self.render.dark_theme_id {
            args.extend([OsStr::new("--dark-theme"), dark_theme_id.as_ref()]);
        }
        args.push(OsStr::new("-"));
        args
    }

    /// Runs the D2 process to generate a diagram
    ///
    /// Executes the D2 binary with a timeout to prevent hanging on malformed input.
    /// Returns the stdout bytes from the D2 process (PNG data when no output file is specified).
    ///
    /// # Arguments
    /// * `ctx` - The render context for the diagram
    /// * `content` - The D2 diagram content
    /// * `args` - Additional arguments for the D2 process
    ///
    /// # Errors
    /// Returns an error if:
    /// - The D2 process fails to spawn
    /// - Writing to stdin fails
    /// - The process exceeds the timeout (30 seconds)
    /// - The D2 compilation fails
    fn run_process(
        &self,
        ctx: &RenderContext,
        content: &str,
        args: Vec<&OsStr>,
    ) -> anyhow::Result<Vec<u8>> {
        let mut child = Command::new(&self.paths.d2_binary)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(args)
            .spawn()
            .with_context(|| {
                format!(
                    "Failed to spawn D2 process. Is D2 installed and available at {}?",
                    self.paths.d2_binary.display()
                )
            })?;

        // Write to stdin safely and close it
        {
            let stdin = child
                .stdin
                .as_mut()
                .context("Failed to open stdin for D2 process")?;
            stdin
                .write_all(content.as_bytes())
                .context("Failed to write D2 diagram content to stdin")?;
        }
        // stdin is automatically closed when it goes out of scope

        // Wait for the process with a timeout
        let Some(status_code) = child.wait_timeout(D2_PROCESS_TIMEOUT)? else {
            // Process exceeded timeout, kill it and reap to prevent zombie
            child.kill().context("Failed to kill D2 process after timeout")?;
            let _ = child.wait(); // Reap the killed process to prevent zombie
            return Err(anyhow!(
                "D2 process timed out after {} seconds while processing diagram ({}, #{}). \
                 The diagram may be too complex or D2 may be hanging. \
                 Consider simplifying the diagram.",
                D2_PROCESS_TIMEOUT.as_secs(),
                ctx.chapter,
                ctx.diagram_index
            ));
        };

        // Collect output after process completes
        let output = child
            .wait_with_output()
            .context("Failed to collect D2 process output")?;

        if status_code.success() {
            Ok(output.stdout)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let indented_stderr = format!("\n{stderr}").replace('\n', "\n  ");
            bail!(
                "Failed to compile D2 diagram ({}, #{}) - D2 exited with status {}:{}",
                ctx.chapter,
                ctx.diagram_index,
                status_code,
                indented_stderr
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdbook::book::SectionNumber;
    use std::path::Path;

    /// Creates a minimal test Backend instance
    fn create_test_backend() -> Backend {
        Backend {
            paths: PathConfig {
                d2_binary: PathBuf::from("d2"),
                output_dir: PathBuf::from("d2"),
                source_dir: PathBuf::from("/test/src"),
            },
            render: RenderConfig {
                layout: None,
                inline: false,
                fonts: None,
                theme_id: None,
                dark_theme_id: None,
            },
        }
    }

    /// Creates a test `RenderContext` with given chapter path
    fn create_test_context<'a>(
        path: &'a Path,
        chapter: &'a str,
        section: Option<&'a SectionNumber>,
        index: usize,
    ) -> RenderContext<'a> {
        RenderContext::new(path, chapter, section, index)
    }

    #[test]
    fn test_calculate_relative_path_root_level_chapter() {
        // Root-level chapter: chapter.md in src/
        let backend = create_test_backend();
        let chapter_path = Path::new("chapter.md");
        let section = SectionNumber(vec![1]);
        let ctx = create_test_context(chapter_path, "Test Chapter", Some(&section), 1);

        let rel_path = backend.calculate_relative_path_for_chapter(&ctx);

        // Root-level: no "../" needed, just "d2/1.1.png"
        assert_eq!(rel_path, PathBuf::from("d2/1.1.png"));
    }

    #[test]
    fn test_calculate_relative_path_one_level_deep() {
        // One level deep: intro/chapter.md
        let backend = create_test_backend();
        let chapter_path = Path::new("intro/chapter.md");
        let section = SectionNumber(vec![1]);
        let ctx = create_test_context(chapter_path, "Test Chapter", Some(&section), 1);

        let rel_path = backend.calculate_relative_path_for_chapter(&ctx);

        // One level: "../d2/1.1.png"
        assert_eq!(rel_path, PathBuf::from("../d2/1.1.png"));
    }

    #[test]
    fn test_calculate_relative_path_two_levels_deep() {
        // Two levels deep: part1/chapter1/file.md
        let backend = create_test_backend();
        let chapter_path = Path::new("part1/chapter1/file.md");
        let section = SectionNumber(vec![1, 1]);
        let ctx = create_test_context(chapter_path, "Test Chapter", Some(&section), 1);

        let rel_path = backend.calculate_relative_path_for_chapter(&ctx);

        // Two levels: "../../d2/1.1.1.png"
        assert_eq!(rel_path, PathBuf::from("../../d2/1.1.1.png"));
    }

    #[test]
    fn test_calculate_relative_path_three_levels_deep() {
        // Three levels deep: a/b/c/chapter.md
        let backend = create_test_backend();
        let chapter_path = Path::new("a/b/c/chapter.md");
        let section = SectionNumber(vec![2, 3, 4]);
        let ctx = create_test_context(chapter_path, "Deep Chapter", Some(&section), 2);

        let rel_path = backend.calculate_relative_path_for_chapter(&ctx);

        // Three levels: "../../../d2/2.3.4.2.png"
        assert_eq!(rel_path, PathBuf::from("../../../d2/2.3.4.2.png"));
    }

    #[test]
    fn test_calculate_relative_path_no_section_number() {
        // Chapter without section number (unnumbered chapter)
        // Now uses path hash for uniqueness instead of "0" prefix
        let backend = create_test_backend();
        let chapter_path = Path::new("appendix/info.md");
        let ctx = create_test_context(chapter_path, "Appendix", None, 1);

        let rel_path = backend.calculate_relative_path_for_chapter(&ctx);
        let rel_str = rel_path.to_string_lossy();

        // One level deep, no section: "../d2/<hash>_1.png"
        assert!(rel_str.starts_with("../d2/"), "Should start with ../d2/, got: {rel_str}");
        assert!(rel_str.ends_with("_1.png"), "Should end with _1.png, got: {rel_str}");
    }

    #[test]
    fn test_calculate_relative_path_custom_output_dir() {
        // Custom output directory
        let mut backend = create_test_backend();
        backend.paths.output_dir = PathBuf::from("diagrams");

        let chapter_path = Path::new("intro/chapter.md");
        let section = SectionNumber(vec![1]);
        let ctx = create_test_context(chapter_path, "Test", Some(&section), 1);

        let rel_path = backend.calculate_relative_path_for_chapter(&ctx);

        // Should use custom output dir: "../diagrams/1.1.png"
        assert_eq!(rel_path, PathBuf::from("../diagrams/1.1.png"));
    }

    #[test]
    fn test_filename_generation() {
        // Test filename generation for various section numbers
        let section1 = SectionNumber(vec![1]);
        let ctx1 = create_test_context(Path::new("test.md"), "Test", Some(&section1), 2);
        assert_eq!(filename(&ctx1), "1.2.png");

        let section2 = SectionNumber(vec![1, 2, 3]);
        let ctx2 = create_test_context(Path::new("test.md"), "Test", Some(&section2), 1);
        assert_eq!(filename(&ctx2), "1.2.3.1.png");

        // No section number - uses path hash for uniqueness
        let ctx3 = create_test_context(Path::new("test.md"), "Test", None, 5);
        let filename3 = filename(&ctx3);
        // Filename should be hash_index.png format (e.g., "a1b2c3d4_5.png")
        assert!(filename3.ends_with("_5.png"), "Expected hash_5.png format, got: {filename3}");
        assert!(filename3.len() > 6, "Filename should have hash prefix: {filename3}");
    }

    #[test]
    fn test_filename_uniqueness_for_unnumbered_chapters() {
        // Two different paths should produce different filenames
        let ctx1 = create_test_context(Path::new("chapter1.md"), "Chapter 1", None, 1);
        let ctx2 = create_test_context(Path::new("chapter2.md"), "Chapter 2", None, 1);

        let filename1 = filename(&ctx1);
        let filename2 = filename(&ctx2);

        assert_ne!(filename1, filename2, "Different paths should produce different filenames");
    }

    #[test]
    fn test_filename_stability_for_same_path() {
        // Same path should always produce the same filename
        let ctx1 = create_test_context(Path::new("test.md"), "Test", None, 1);
        let ctx2 = create_test_context(Path::new("test.md"), "Test", None, 1);

        assert_eq!(filename(&ctx1), filename(&ctx2), "Same path should produce same filename");
    }

    #[test]
    fn test_relative_file_path() {
        let backend = create_test_backend();
        let section = SectionNumber(vec![1, 2]);
        let ctx = create_test_context(Path::new("test.md"), "Test", Some(&section), 3);

        let rel_path = backend.relative_file_path(&ctx);

        // Should be output_dir + filename
        assert_eq!(rel_path, PathBuf::from("d2/1.2.3.png"));
    }

    #[test]
    fn test_filepath_construction() {
        let backend = create_test_backend();
        let section = SectionNumber(vec![2]);
        let ctx = create_test_context(Path::new("test.md"), "Test", Some(&section), 1);

        let filepath = backend.filepath(&ctx);

        // Should be source_dir + output_dir + filename
        assert_eq!(filepath, PathBuf::from("/test/src/d2/2.1.png"));
    }
}
