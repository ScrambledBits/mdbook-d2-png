# TODO

## Current Tasks
- None currently pending

## Completed (2025-11-22)
- ✅ Fixed compilation error: Added `Clone` derive to `Fonts` struct (src/config.rs:20)
- ✅ Removed unused `CowStr` import from src/lib.rs:16
- ✅ Fixed clippy::uninlined_format_args errors (5 occurrences in backend.rs and lib.rs)
- ✅ Verified build stability (debug and release builds pass)
- ✅ Verified clippy passes with only pedantic/nursery warnings

## Completed (2025-11-19)
- ✅ Verified build stability across all configurations
- ✅ Updated all documentation (CHANGELOG.md, CLAUDE.md)
- ✅ Bumped version to 0.3.7-png.3

## Known Issues
- Test suite has pre-existing toml dependency conflict (E0464) - does not affect main builds
- Clippy pedantic warnings exist in codebase (not blocking, style-related)

## Future Improvements (Non-Critical)
- Consider resolving toml dependency conflict for cleaner test builds
- Address clippy pedantic warnings for improved code quality
- Add doc comment to Fonts struct explaining Clone semantics
- Add unit test verifying `Backend: Clone` compiles
- Consider using `Arc<Fonts>` if heavy cloning becomes an issue (unlikely with PathBuf)
