//! [D2] diagram generator [`Preprocessor`] library for [`MdBook`](https://rust-lang.github.io/mdBook/).

#![deny(
    clippy::all,
    missing_debug_implementations,
    missing_copy_implementations,
    missing_docs
)]
#![warn(clippy::pedantic, clippy::nursery)]

use std::path::PathBuf;
use std::sync::Arc;

use log::error;
use mdbook::book::{Book, Chapter, SectionNumber};
use mdbook::errors::Error;
use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::BookItem;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use pulldown_cmark_to_cmark::cmark;
use rayon::prelude::*;

mod backend;
use backend::{Backend, RenderContext};

mod config;

/// The name of this preprocessor
const PREPROCESSOR_NAME: &str = "d2-png";

/// The code block language identifier for D2 diagrams
const D2_CODE_BLOCK_LANG: &str = "d2";

/// Maximum number of concurrent D2 processes
///
/// D2 is CPU-intensive, so we cap concurrent processes to prevent resource exhaustion.
/// This value balances parallelism with system resource constraints.
const MAX_CONCURRENT_D2_PROCESSES: usize = 8;

/// [D2] diagram generator [`Preprocessor`] for [`MdBook`](https://rust-lang.github.io/mdBook/).
#[derive(Default, Clone, Copy, Debug)]
pub struct D2;

/// A render job for a D2 diagram
///
/// Contains all information needed to render a diagram in parallel.
#[derive(Debug, Clone)]
struct RenderJob {
    /// Path to the chapter file (for relative path calculation)
    chapter_path: PathBuf,
    /// Name of the chapter (for error messages)
    chapter_name: String,
    /// Section number (for filename generation)
    section: Option<SectionNumber>,
    /// D2 diagram content
    content: String,
    /// 1-based index of this diagram within its chapter
    diagram_index: usize,
}

impl Preprocessor for D2 {
    fn name(&self) -> &'static str {
        PREPROCESSOR_NAME
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let backend = Arc::new(Backend::from_context(ctx));

        // Pass 1: Collect all render jobs from all chapters
        let mut chapter_jobs: Vec<(usize, Vec<RenderJob>)> = Vec::new();
        let mut chapter_indices: Vec<usize> = Vec::new();

        book.for_each_mut(|section| {
            if let BookItem::Chapter(chapter) = section {
                let jobs = collect_render_jobs(chapter);
                if !jobs.is_empty() {
                    chapter_indices.push(chapter_jobs.len());
                    chapter_jobs.push((chapter_indices.len() - 1, jobs));
                }
            }
        });

        // Flatten all jobs for parallel processing
        let all_jobs: Vec<(usize, usize, RenderJob)> = chapter_jobs
            .into_iter()
            .flat_map(|(chapter_idx, jobs)| {
                jobs.into_iter()
                    .enumerate()
                    .map(move |(job_idx, job)| (chapter_idx, job_idx, job))
            })
            .collect();

        // Pass 2: Render all diagrams in parallel with bounded concurrency
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_cpus().min(MAX_CONCURRENT_D2_PROCESSES))
            .build()
            .expect("Failed to create thread pool for D2 rendering");

        let rendered_results: Vec<(usize, usize, Result<Vec<Event<'static>>, String>)> =
            pool.install(|| {
                all_jobs
                    .into_par_iter()
                    .map(|(chapter_idx, job_idx, job)| {
                        let render_ctx = RenderContext::new(
                            &job.chapter_path,
                            &job.chapter_name,
                            job.section.as_ref(),
                            job.diagram_index,
                        );

                        let result = backend
                            .render(&render_ctx, &job.content)
                            .map_err(|e| e.to_string());

                        (chapter_idx, job_idx, result)
                    })
                    .collect()
            });

        // Group results by chapter for stitching
        let mut results_by_chapter: std::collections::HashMap<usize, Vec<(usize, Vec<Event<'static>>)>> =
            std::collections::HashMap::new();

        for (chapter_idx, job_idx, result) in rendered_results {
            let events = match result {
                Ok(events) => events,
                Err(e) => {
                    error!("Failed to render D2 diagram: {e}");
                    Vec::new()
                }
            };
            results_by_chapter
                .entry(chapter_idx)
                .or_default()
                .push((job_idx, events));
        }

        // Sort results within each chapter by job index
        for results in results_by_chapter.values_mut() {
            results.sort_by_key(|(idx, _)| *idx);
        }

        // Pass 3: Stitch results back into chapters
        let mut chapter_counter = 0;
        book.for_each_mut(|section| {
            if let BookItem::Chapter(chapter) = section {
                let chapter_results = results_by_chapter.remove(&chapter_counter);
                chapter_counter += 1;

                let rendered_events: Vec<Vec<Event<'static>>> = chapter_results
                    .map(|mut results| {
                        results.sort_by_key(|(idx, _)| *idx);
                        results.into_iter().map(|(_, events)| events).collect()
                    })
                    .unwrap_or_default();

                let events = stitch_events(
                    chapter,
                    Parser::new_ext(&chapter.content, Options::all()),
                    rendered_events,
                );

                let mut buf = String::with_capacity(chapter.content.len() + 128);
                cmark(events, &mut buf)
                    .expect("Failed to convert markdown events back to markdown");
                chapter.content = buf;
            }
        });

        Ok(book)
    }
}

/// Returns the number of available CPUs
fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(1)
}

/// Collects all D2 render jobs from a chapter
///
/// Scans through markdown events to find D2 code blocks and creates render jobs for each.
fn collect_render_jobs(chapter: &Chapter) -> Vec<RenderJob> {
    let source_path = chapter
        .source_path
        .as_ref()
        .expect("Chapter source path should always be set by mdBook");

    let events = Parser::new_ext(&chapter.content, Options::all());

    let mut jobs = Vec::new();
    let mut in_block = false;
    let mut diagram_content = String::new();
    let mut diagram_index = 0usize;

    for event in events {
        if is_d2_block_start(&event) {
            in_block = true;
            diagram_content.clear();
            diagram_index += 1;
        } else if in_block {
            if let Event::Text(content) = &event {
                diagram_content.push_str(content);
            } else if matches!(event, Event::End(TagEnd::CodeBlock)) {
                in_block = false;
                jobs.push(RenderJob {
                    chapter_path: source_path.clone(),
                    chapter_name: chapter.name.clone(),
                    section: chapter.number.clone(),
                    content: std::mem::take(&mut diagram_content),
                    diagram_index,
                });
            }
        }
    }

    jobs
}

/// Checks if an event marks the start of a D2 code block
fn is_d2_block_start(event: &Event) -> bool {
    matches!(
        event,
        Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) if lang.as_ref() == D2_CODE_BLOCK_LANG
    )
}

/// Stitches pre-rendered diagram events back into the markdown event stream
///
/// Replaces D2 code blocks with their pre-rendered image events in order.
fn stitch_events<'a>(
    _chapter: &'a Chapter,
    events: impl Iterator<Item = Event<'a>> + 'a,
    mut rendered_events: Vec<Vec<Event<'static>>>,
) -> impl Iterator<Item = Event<'a>> + 'a {
    // Reverse so we can pop from the back (more efficient than removing from front)
    rendered_events.reverse();

    let mut in_block = false;
    let mut pending_events: Option<Vec<Event<'static>>> = None;

    // Use a closure to process events with state
    let mut result_events: Vec<Event<'a>> = Vec::new();

    for event in events {
        // First, emit any pending events from a previous diagram
        if let Some(events) = pending_events.take() {
            result_events.extend(events);
        }

        if is_d2_block_start(&event) {
            in_block = true;
            // Skip the start event
        } else if in_block {
            if let Event::Text(_) = &event {
                // Skip text content (the D2 code)
            } else if matches!(event, Event::End(TagEnd::CodeBlock)) {
                in_block = false;
                // Pop the next rendered result (in reverse order)
                if let Some(events) = rendered_events.pop() {
                    pending_events = Some(events);
                }
            }
        } else {
            result_events.push(event);
        }
    }

    // Emit any remaining pending events
    if let Some(events) = pending_events {
        result_events.extend(events);
    }

    result_events.into_iter()
}
