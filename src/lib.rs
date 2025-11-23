//! [D2] diagram generator [`Preprocessor`] library for [`MdBook`](https://rust-lang.github.io/mdBook/).

#![deny(
    clippy::all,
    missing_debug_implementations,
    missing_copy_implementations,
    missing_docs
)]
#![warn(clippy::pedantic, clippy::nursery)]

use log::error;
use mdbook::book::{Book, Chapter};
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::BookItem;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use pulldown_cmark_to_cmark::cmark;

mod backend;
use backend::{Backend, RenderContext};

mod config;

/// The name of this preprocessor
const PREPROCESSOR_NAME: &str = "d2-png";

/// The code block language identifier for D2 diagrams
const D2_CODE_BLOCK_LANG: &str = "d2";

/// [D2] diagram generator [`Preprocessor`] for [`MdBook`](https://rust-lang.github.io/mdBook/).
#[derive(Default, Clone, Copy, Debug)]
pub struct D2;

impl Preprocessor for D2 {
    fn name(&self) -> &'static str {
        PREPROCESSOR_NAME
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let backend = Backend::from_context(ctx);

        book.for_each_mut(|section| {
            if let BookItem::Chapter(chapter) = section {
                let events = process_events(
                    &backend,
                    chapter,
                    Parser::new_ext(&chapter.content, Options::all()),
                );

                // create a buffer in which we can place the markdown
                let mut buf = String::with_capacity(chapter.content.len() + 128);

                // convert it back to markdown and replace the original chapter's content
                cmark(events, &mut buf)
                    .expect("Failed to convert markdown events back to markdown");
                chapter.content = buf;
            }
        });

        Ok(book)
    }
}

/// Processor for D2 code blocks within markdown events
///
/// Manages state while processing a stream of markdown events, detecting D2 code blocks,
/// accumulating their content, and rendering them to PNG images.
///
/// # Design Rationale
///
/// This processor is implemented as a struct rather than inline closure logic for several reasons:
/// - **Testability**: Each method can be unit tested independently
/// - **Clarity**: State transitions are explicit with named methods
/// - **Maintainability**: Logic is easier to understand and modify
/// - **Single Responsibility**: Each method has one clear purpose
///
/// While a closure with local variables would be more concise, this approach provides
/// better long-term maintainability and makes the code more approachable for contributors.
struct D2BlockProcessor<'a> {
    backend: &'a Backend,
    chapter: &'a Chapter,
    in_block: bool,
    diagram_content: String,
    diagram_index: usize,
}

impl<'a> D2BlockProcessor<'a> {
    /// Creates a new D2 block processor
    fn new(backend: &'a Backend, chapter: &'a Chapter) -> Self {
        Self {
            backend,
            chapter,
            in_block: false,
            diagram_content: String::new(),
            diagram_index: 0,
        }
    }

    /// Processes a single markdown event, potentially transforming it
    ///
    /// Returns a vector of events to emit (may be empty, one, or multiple events)
    fn process_event(&mut self, event: Event<'a>) -> Vec<Event<'a>> {
        if self.is_d2_block_start(&event) {
            self.start_block();
            vec![]
        } else if self.in_block && self.is_text_event(&event) {
            self.accumulate_content(&event);
            vec![]
        } else if self.in_block && self.is_block_end(&event) {
            self.end_block()
        } else {
            vec![event]
        }
    }

    /// Checks if an event marks the start of a D2 code block
    ///
    /// Matches both `CowStr::Borrowed` and `CowStr::Boxed` variants to ensure
    /// all D2 blocks are detected regardless of how the parser creates the string.
    fn is_d2_block_start(&self, event: &Event) -> bool {
        matches!(
            event,
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) if lang.as_ref() == D2_CODE_BLOCK_LANG
        )
    }

    /// Checks if an event is a text event
    fn is_text_event(&self, event: &Event) -> bool {
        matches!(event, Event::Text(_))
    }

    /// Checks if an event marks the end of a code block
    fn is_block_end(&self, event: &Event) -> bool {
        matches!(event, Event::End(TagEnd::CodeBlock))
    }

    /// Begins processing a new D2 code block
    fn start_block(&mut self) {
        self.in_block = true;
        self.diagram_content.clear();
        self.diagram_index += 1;
    }

    /// Accumulates text content from within a D2 code block
    ///
    /// Note: Windows CRLF line endings can cause a code block to consist of
    /// multiple Text events, so we need to buffer them.
    /// See: https://github.com/raphlinus/pulldown-cmark/issues/507
    fn accumulate_content(&mut self, event: &Event) {
        if let Event::Text(content) = event {
            self.diagram_content.push_str(content);
        }
    }

    /// Completes processing of a D2 code block and renders it
    ///
    /// Returns the events to replace the code block (either rendered image or empty on error)
    fn end_block(&mut self) -> Vec<Event<'static>> {
        self.in_block = false;

        let source_path = self.chapter.source_path.as_ref()
            .expect("Chapter source path should always be set by mdBook");

        let render_context = RenderContext::new(
            source_path,
            &self.chapter.name,
            self.chapter.number.as_ref(),
            self.diagram_index,
        );

        self.backend
            .render(&render_context, &self.diagram_content)
            .unwrap_or_else(|e| {
                // If we cannot render the diagram, log the error and return an empty block
                error!("Failed to render D2 diagram: {e}");
                vec![]
            })
    }
}

fn process_events<'a>(
    backend: &'a Backend,
    chapter: &'a Chapter,
    events: impl Iterator<Item = Event<'a>> + 'a,
) -> impl Iterator<Item = Event<'a>> + 'a {
    let mut processor = D2BlockProcessor::new(backend, chapter);
    events.flat_map(move |event| processor.process_event(event))
}
