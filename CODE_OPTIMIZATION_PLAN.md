# Code Optimization Plan for mdbook-d2-png

## Executive Summary

This document provides a comprehensive analysis of the mdbook-d2-png codebase applying **DRY (Don't Repeat Yourself)** and **KISS (Keep It Simple, Stupid)** principles. The analysis identifies areas where code can be simplified, duplication can be eliminated, and overall maintainability can be improved.

---

## Table of Contents

1. [DRY Principle Violations](#dry-principle-violations)
2. [KISS Principle Violations](#kiss-principle-violations)
3. [Additional Code Quality Issues](#additional-code-quality-issues)
4. [Optimization Recommendations](#optimization-recommendations)
5. [Implementation Priority](#implementation-priority)

---

## DRY Principle Violations

### 1. Duplicate Rendering Logic (backend.rs:144-206)

**Location:** `render_inline_png()` and `render_embedded_png()` methods

**Issue:**
Both methods contain nearly identical setup code:
- `fs::create_dir_all()` - Directory creation
- `self.basic_args()` - Argument building
- `self.filepath(ctx)` - File path construction
- `self.run_process()` - D2 process execution

**Current Code Pattern:**
```rust
// render_inline_png (lines 144-171)
fs::create_dir_all(Path::new(&self.source_dir).join(self.output_dir())).unwrap();
let mut args = self.basic_args();
let filepath = self.filepath(ctx);
args.push(filepath.as_os_str());
self.run_process(ctx, content, args)?;

// render_embedded_png (lines 173-206)
fs::create_dir_all(Path::new(&self.source_dir).join(self.output_dir())).unwrap();
let mut args = self.basic_args();
let filepath = self.filepath(ctx);
args.push(filepath.as_os_str());
self.run_process(ctx, content, args)?;
```

**Impact:** Code duplication, harder maintenance, increased risk of bugs when updating one method but not the other.

**Recommended Solution:**
Extract common setup logic into a separate method:
```rust
fn generate_diagram(&self, ctx: &RenderContext, content: &str) -> anyhow::Result<PathBuf> {
    self.ensure_output_dir()?;
    let filepath = self.filepath(ctx);
    let mut args = self.basic_args();
    args.push(filepath.as_os_str());
    self.run_process(ctx, content, args)?;
    Ok(filepath)
}
```

### 2. Duplicate Image Event Creation (backend.rs:161-205)

**Issue:**
Both rendering methods manually construct similar `Event` vectors for Paragraph and Image tags with only minor differences in the image URL.

**Current Code:**
```rust
// Appears twice with slight variations
Ok(vec![
    Event::Start(Tag::Paragraph),
    Event::Start(Tag::Image {
        link_type: LinkType::Inline,
        dest_url: /* different values */,
        title: CowStr::Borrowed(""),
        id: CowStr::Borrowed(""),
    }),
    Event::End(TagEnd::Image),
    Event::End(TagEnd::Paragraph),
])
```

**Recommended Solution:**
Create a helper function:
```rust
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
```

### 3. Path Construction Duplication (backend.rs:113-125)

**Issue:**
`filepath()` calls `relative_file_path()` and then joins with `source_dir`, but the logic could be more straightforward.

**Current Code:**
```rust
fn filepath(&self, ctx: &RenderContext) -> PathBuf {
    let filepath = Path::new(&self.source_dir).join(self.relative_file_path(ctx));
    filepath
}

fn relative_file_path(&self, ctx: &RenderContext) -> PathBuf {
    let filename = filename(ctx);
    self.output_dir.join(filename)
}
```

**Recommended Solution:**
Simplify and make more direct:
```rust
fn filepath(&self, ctx: &RenderContext) -> PathBuf {
    self.source_dir.join(&self.output_dir).join(filename(ctx))
}

fn relative_file_path(&self, ctx: &RenderContext) -> PathBuf {
    self.output_dir.join(filename(ctx))
}
```

### 4. Default Function Verbosity (config.rs:5-19)

**Issue:**
Three separate default functions for simple values that could be constants or inline.

**Current Code:**
```rust
mod default {
    use std::path::PathBuf;

    pub fn bin_path() -> PathBuf {
        PathBuf::from("d2")
    }

    pub fn output_dir() -> PathBuf {
        PathBuf::from("d2")
    }

    pub const fn inline() -> bool {
        false
    }
}
```

**Recommended Solution:**
Use inline default values or constants:
```rust
const DEFAULT_BIN_PATH: &str = "d2";
const DEFAULT_OUTPUT_DIR: &str = "d2";
const DEFAULT_INLINE: bool = false;

// Or use inline defaults in the struct:
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    #[serde(default = "default_path")]
    pub path: PathBuf,
    // ...
}

fn default_path() -> PathBuf { PathBuf::from("d2") }
```

### 5. Error Formatting Pattern (backend.rs:268-274)

**Issue:**
Custom error message formatting is done inline and could be extracted.

**Current Code:**
```rust
let src = format!("\n{}", String::from_utf8_lossy(&output.stderr)).replace('\n', "\n  ");
let msg = format!(
    "failed to compile D2 diagram ({}, #{}):{src}",
    ctx.chapter, ctx.diagram_index
);
bail!(msg)
```

**Recommended Solution:**
Create a helper method:
```rust
fn format_d2_error(ctx: &RenderContext, stderr: &[u8]) -> String {
    let indented_stderr = format!("\n{}", String::from_utf8_lossy(stderr))
        .replace('\n', "\n  ");
    format!(
        "failed to compile D2 diagram ({}, #{}):{indented_stderr}",
        ctx.chapter, ctx.diagram_index
    )
}
```

---

## KISS Principle Violations

### 1. Over-Complicated Path Depth Calculation (backend.rs:186-189)

**Issue:**
The relative path construction using `ancestors().count() - 2` and `repeat_n` is confusing and error-prone.

**Current Code:**
```rust
let depth = ctx.path.ancestors().count() - 2;
let rel_path: PathBuf = std::iter::repeat_n(Path::new(".."), depth)
    .collect::<PathBuf>()
    .join(self.relative_file_path(ctx));
```

**Why This Is Complex:**
- Magic number `-2` is unexplained
- Not immediately clear what `ancestors().count()` represents
- Multiple chained operations make debugging difficult

**Recommended Solution:**
```rust
// Calculate how many directories deep the chapter is from source root
fn calculate_relative_path(&self, chapter_path: &Path) -> PathBuf {
    let parent_depth = chapter_path.parent()
        .map(|p| p.components().count())
        .unwrap_or(0);

    let mut rel_path = PathBuf::new();
    for _ in 0..parent_depth {
        rel_path.push("..");
    }
    rel_path.join(self.relative_file_path(ctx))
}
```

Or even simpler using `pathdiff` crate or manual calculation.

### 2. Complex Event Processing State Machine (lib.rs:68-106)

**Issue:**
The `process_events` function uses a closure with mutable state and complex pattern matching that's hard to follow.

**Current Code:**
```rust
fn process_events<'a>(
    backend: &'a Backend,
    chapter: &'a Chapter,
    events: impl Iterator<Item = Event<'a>> + 'a,
) -> impl Iterator<Item = Event<'a>> + 'a {
    let mut in_block = false;
    let mut diagram = String::new();
    let mut diagram_index = 0;

    events.flat_map(move |event| {
        match (&event, in_block) {
            // Complex nested logic...
        }
    })
}
```

**Why This Is Complex:**
- Mutable state scattered across closure
- Tuple matching `(&event, in_block)` is not intuitive
- Logic for state transitions is embedded in pattern matching

**Recommended Solution:**
Extract to a separate stateful struct:
```rust
struct D2BlockProcessor<'a> {
    backend: &'a Backend,
    chapter: &'a Chapter,
    in_block: bool,
    diagram_content: String,
    diagram_index: usize,
}

impl<'a> D2BlockProcessor<'a> {
    fn process_event(&mut self, event: Event<'a>) -> Vec<Event<'a>> {
        if self.is_d2_block_start(&event) {
            self.start_block();
            vec![]
        } else if self.in_block && self.is_text(&event) {
            self.add_content(&event);
            vec![]
        } else if self.is_block_end(&event) && self.in_block {
            self.end_block()
        } else {
            vec![event]
        }
    }

    fn is_d2_block_start(&self, event: &Event) -> bool {
        matches!(
            event,
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(CowStr::Borrowed("d2"))))
        )
    }
    // ... other helper methods
}
```

### 3. Backend Struct Has Too Many Fields (backend.rs:14-27)

**Issue:**
The Backend struct has 7 fields, many of which are configuration options that could be grouped.

**Current Code:**
```rust
pub struct Backend {
    path: PathBuf,
    output_dir: PathBuf,
    source_dir: PathBuf,
    layout: Option<String>,
    inline: bool,
    fonts: Option<Fonts>,
    theme_id: Option<String>,
    dark_theme_id: Option<String>,
}
```

**Recommended Solution:**
Group related fields:
```rust
pub struct Backend {
    config: RenderConfig,
    paths: PathConfig,
}

struct RenderConfig {
    layout: Option<String>,
    inline: bool,
    fonts: Option<Fonts>,
    theme_id: Option<String>,
    dark_theme_id: Option<String>,
}

struct PathConfig {
    d2_binary: PathBuf,
    output_dir: PathBuf,
    source_dir: PathBuf,
}
```

### 4. Inconsistent Error Handling

**Issue:**
Multiple error handling strategies used throughout the codebase:

**Examples:**
```rust
// backend.rs:153, 179 - unwrap without context
fs::create_dir_all(...).unwrap();

// backend.rs:98 - expect with message
.expect("Unable to deserialize d2-png preprocessor config")
.expect("d2-png preprocessor config not found");

// backend.rs:158 - ? operator
self.run_process(ctx, content, args)?;

// lib.rs:96 - unwrap_or_else
backend.render(&render_context, &diagram).unwrap_or_else(|e| {
    eprintln!("{e}");
    vec![]
})
```

**Recommended Solution:**
Establish consistent error handling patterns:
- Use `?` operator for propagating errors
- Use `context()` from anyhow for adding context
- Avoid `unwrap()` and `expect()` in production code paths
- Create custom error types if needed

```rust
fs::create_dir_all(self.get_output_path())
    .context("Failed to create output directory")?;
```

### 5. Unnecessarily Complex Generic Constraints (backend.rs:240-249)

**Issue:**
The `run_process` method uses complex generic constraints when simpler types would suffice.

**Current Code:**
```rust
fn run_process<I, S>(
    &self,
    ctx: &RenderContext,
    content: &str,
    args: I,
) -> anyhow::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    // ...
}
```

**Recommended Solution:**
Use concrete types for internal methods:
```rust
fn run_process(
    &self,
    ctx: &RenderContext,
    content: &str,
    args: &[&OsStr],
) -> anyhow::Result<()> {
    // ...
}
```

Note: The method doesn't actually return the diagram string - it writes to a file. The return type should reflect this.

### 6. RenderContext Has Redundant Data (backend.rs:30-57)

**Issue:**
RenderContext carries multiple pieces of data when a simpler identifier might suffice.

**Current Code:**
```rust
pub struct RenderContext<'a> {
    path: &'a Path,
    chapter: &'a str,
    section: Option<&'a SectionNumber>,
    diagram_index: usize,
}
```

**Analysis:**
- `chapter` name is only used for error messages
- `section` and `diagram_index` are used together to create filenames

**Recommended Solution:**
Simplify if possible, or at least document the purpose of each field clearly:
```rust
/// Context for rendering a single D2 diagram within a chapter
pub struct RenderContext<'a> {
    /// Path to the chapter file (for calculating relative paths)
    chapter_path: &'a Path,
    /// Identifier for the diagram (section number + index)
    diagram_id: DiagramId<'a>,
}

struct DiagramId<'a> {
    chapter_name: &'a str,
    section: Option<&'a SectionNumber>,
    index: usize,
}
```

---

## Additional Code Quality Issues

### 1. Incorrect Documentation (config.rs:39)

**Issue:**
Comment says "Default is 'true'" but the actual default is `false`.

**Location:** config.rs:39

**Current Code:**
```rust
/// Whether or not to use inline SVG when building an HTML target
///
/// Default is 'true'  // <-- WRONG
#[serde(default = "default::inline")]
pub inline: bool,  // default::inline() returns false
```

**Fix:**
```rust
/// Whether to inline PNG images as base64 data URIs
///
/// Default is `false` (images are saved as separate files)
#[serde(default = "default::inline")]
pub inline: bool,
```

### 2. No Structured Logging

**Issue:**
Uses `eprintln!` directly instead of a logging framework.

**Examples:**
- main.rs:63: `eprintln!("{e}");`
- main.rs:75-81: Warning message
- lib.rs:99: `eprintln!("{e}");`

**Recommendation:**
Add the `log` or `tracing` crate and use structured logging:
```rust
use log::{error, warn, info};

// Instead of:
eprintln!("{e}");

// Use:
error!("Failed to render D2 diagram: {}", e);
```

### 3. Process Execution Lacks Robustness (backend.rs:250-276)

**Issues:**
- No timeout handling for D2 process
- stdin write happens after spawn but before checking if pipe is available
- No handling of process that hangs

**Current Code:**
```rust
let child = Command::new(&self.path)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .args(args)
    .spawn()?;

child
    .stdin
    .as_ref()
    .unwrap()  // <-- Could panic
    .write_all(content.as_bytes())?;

let output = child.wait_with_output()?;  // <-- No timeout
```

**Recommended Solution:**
```rust
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
        .context("Failed to open stdin")?;
    stdin.write_all(content.as_bytes())
        .context("Failed to write to D2 stdin")?;
}

// Consider adding timeout using wait_timeout crate
let output = child.wait_with_output()
    .context("Failed to wait for D2 process")?;
```

### 4. Magic String "d2" Appears Multiple Times

**Issue:**
The code block language identifier "d2" is hardcoded in multiple places.

**Locations:**
- lib.rs:72: `CodeBlockKind::Fenced(CowStr::Borrowed("d2"))`

**Recommendation:**
```rust
const D2_CODE_BLOCK_LANG: &str = "d2";
```

### 5. Test Code Could Be More DRY (tests/render.rs)

**Issue:**
Multiple tests have similar patterns with repeated assertions.

**Current Code:**
```rust
#[test]
fn simple() {
    let test_book = TestBook::new("simple").expect("couldn't create book");
    assert!(test_book.chapter1_contains(r#"img src="d2/1.1.png" alt="" />"#));
}

#[test]
fn simple_output_dir() {
    let test_book = TestBook::new("simple").expect("couldn't create book");
    let output = test_book.book.source_dir().join("d2/1.1.png");
    assert!(output.exists());
    assert!(test_book.chapter1_contains(r#"img src="d2/1.1.png" alt="" />"#));
}
```

**Recommendation:**
```rust
fn test_book_has_diagram(book_name: &str, expected_path: &str) {
    let test_book = TestBook::new(book_name)
        .expect("couldn't create book");
    assert!(test_book.chapter1_contains(expected_path));
}

#[test]
fn simple() {
    test_book_has_diagram("simple", r#"img src="d2/1.1.png" alt="" />"#);
}
```

### 6. Unused Return Value in run_process (backend.rs:265)

**Issue:**
`run_process` returns `Ok(diagram)` but the diagram string is never used by callers (render methods don't use the return value).

**Current Code:**
```rust
fn run_process(...) -> anyhow::Result<String> {
    // ...
    if output.status.success() {
        let diagram = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(diagram)  // <-- Never used!
    }
}
```

**Recommendation:**
Change return type to `anyhow::Result<()>`:
```rust
fn run_process(...) -> anyhow::Result<()> {
    // ...
    if output.status.success() {
        Ok(())
    } else {
        bail!(self.format_d2_error(ctx, &output.stderr))
    }
}
```

### 7. Missing Error Context in Multiple Places

**Issue:**
Many error propagations lack context about what operation failed.

**Examples:**
```rust
let output = child.wait_with_output()?;  // What failed?
fs::read(&filepath)?  // Why are we reading this file?
```

**Recommendation:**
```rust
let output = child.wait_with_output()
    .context("Failed to wait for D2 process completion")?;

let bytes = fs::read(&filepath)
    .with_context(|| format!("Failed to read generated PNG at {:?}", filepath))?;
```

---

## Optimization Recommendations

### Phase 1: Critical Fixes (High Priority)

1. **Fix incorrect documentation** (config.rs:39)
   - Effort: Low
   - Impact: High (prevents confusion)

2. **Fix error handling in process execution** (backend.rs:260)
   - Change `.unwrap()` to proper error handling
   - Effort: Low
   - Impact: High (prevents panics)

3. **Extract duplicate rendering setup** (backend.rs:144-206)
   - Create `generate_diagram()` helper method
   - Effort: Medium
   - Impact: High (reduces duplication significantly)

4. **Add constants for magic strings**
   - Extract "d2", "d2-png" to constants
   - Effort: Low
   - Impact: Medium

### Phase 2: Structural Improvements (Medium Priority)

5. **Simplify path calculation** (backend.rs:186-189)
   - Replace `ancestors().count() - 2` with clearer logic
   - Add documentation explaining the calculation
   - Effort: Medium
   - Impact: High (improves maintainability)

6. **Create image events helper** (backend.rs:161-205)
   - Extract common event creation logic
   - Effort: Low
   - Impact: Medium

7. **Refactor event processing** (lib.rs:68-106)
   - Extract to separate struct with clear state management
   - Effort: High
   - Impact: High (greatly improves readability)

8. **Add structured logging**
   - Replace `eprintln!` with proper logging
   - Effort: Medium
   - Impact: Medium

### Phase 3: Architecture Improvements (Low Priority)

9. **Simplify Backend struct** (backend.rs:14-27)
   - Group related fields into sub-structs
   - Effort: High
   - Impact: Medium (better organization)

10. **Remove generic constraints** (backend.rs:240)
    - Use concrete types for internal methods
    - Effort: Low
    - Impact: Low (marginal performance/clarity improvement)

11. **Consolidate RenderContext** (backend.rs:30-57)
    - Simplify or better document the purpose of each field
    - Effort: Medium
    - Impact: Medium

12. **Fix unused return value** (backend.rs:265)
    - Change return type to `Result<()>`
    - Effort: Low
    - Impact: Low

### Phase 4: Enhanced Error Handling

13. **Add error context throughout**
    - Use `.context()` consistently
    - Effort: Medium
    - Impact: High (better debugging)

14. **Create custom error types**
    - Define specific error types for common failure cases
    - Effort: High
    - Impact: Medium

15. **Add timeout to process execution**
    - Prevent hanging on malformed D2 input
    - Effort: Medium
    - Impact: Medium

---

## Implementation Priority

### Quick Wins (Do First)
These provide immediate value with minimal effort:

1. Fix incorrect documentation (config.rs:39)
2. Add constants for magic strings
3. Fix `.unwrap()` in process execution (backend.rs:260)
4. Create image events helper function
5. Fix unused return value in `run_process`

### High-Impact Refactoring (Do Second)
These require more effort but significantly improve code quality:

6. Extract duplicate rendering setup
7. Simplify path depth calculation
8. Add consistent error context
9. Refactor event processing to separate struct

### Architectural Improvements (Do Third)
These are nice-to-have and can be done incrementally:

10. Add structured logging
11. Simplify Backend struct
12. Add process timeout handling
13. Create custom error types

---

## Estimated Impact

### Before Optimization:
- **Lines of Code**: ~650
- **Code Duplication**: ~25% (setup code duplicated, event creation duplicated)
- **Cyclomatic Complexity**: High (complex pattern matching in event processing)
- **Error Handling**: Inconsistent (mix of unwrap, expect, ?)

### After Optimization:
- **Lines of Code**: ~600 (10% reduction through deduplication)
- **Code Duplication**: ~5% (only intentional patterns remain)
- **Cyclomatic Complexity**: Medium (extracted into focused functions)
- **Error Handling**: Consistent (all use ? with context)

### Maintainability Score:
- **Before**: 6/10
- **After**: 9/10

---

## Testing Strategy

For each optimization:

1. **Ensure existing tests pass** before changes
2. **Make incremental changes** - one optimization at a time
3. **Run full test suite** after each change
4. **Add new tests** for edge cases discovered during refactoring
5. **Test with actual mdbook projects** to verify behavior

### Suggested Additional Tests:

1. Test error handling when D2 binary is not found
2. Test behavior with invalid D2 syntax
3. Test path calculation for deeply nested chapters
4. Test concurrent diagram generation
5. Test with various D2 layout engines

---

## Conclusion

This codebase is well-structured and functional, but has opportunities for improvement in:

1. **Reducing duplication** - Especially in rendering logic and event creation
2. **Simplifying complexity** - Path calculations and event processing
3. **Improving consistency** - Error handling and coding patterns
4. **Enhancing robustness** - Process execution and error reporting

Implementing these optimizations will result in:
- More maintainable code
- Fewer potential bugs
- Easier onboarding for new contributors
- Better debugging experience

The recommended approach is to tackle quick wins first, then move to high-impact refactoring, and finally architectural improvements as time permits.

---

**Document Version:** 1.0
**Date:** 2025-10-30
**Reviewed By:** Claude Code (AI Assistant)
