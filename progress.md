# Project Progress Roadmap: Terminal Image Rendering for `termbrowse`

This document defines the official 12-stage development roadmap, project tracker, milestone planner, testing checklist, and release plan for adding image rendering capabilities to `termbrowse`.

---

## Global Project Rules

1. **Architecture Group Alignment**: Development is organized into 3 logical Groups (Group A: Foundation, Group B: Dynamic Layout & Viewport Integration, Group C: Polish & Interactive Features). Within each Group, sub-stages represent tightly coupled components built as a coherent system.
2. **Sequential Progression**: Complete each Group and its internal milestones in strict order.
3. **No Broken States**: Every completed milestone must compile cleanly with 0 errors and zero regressions.
4. **Three-Tier Cache**: Always preserve full-resolution `DynamicImage` objects in memory (Tier 2) to allow instant re-scaling during terminal resize events.
5. **Dynamic Document Reflow**: Initial document layout uses alt-text placeholders; arrival of async decoded images triggers incremental layout reflow.
6. **Strict Anti-Push Rule**: FORBIDDEN to push any commits or code to GitHub (`git push` is prohibited). All work remains local on `feature/dev`.
7. **Target Verification Site**: `https://kalalawyer.com`.

---

# Group A – Image Engine Foundation (Stages 1 – 4)

### Group Objective
Build a robust, asynchronous image acquisition, parsing, decoding, and three-tier caching foundation capable of handling PNG, JPEG, WebP, and GIF (Frame 0) images with strict rate limiting and size boundaries.

---

## Stage 1: Image Parsing & Alt-Text Extraction

### Primary Objective
Refactor `parse.rs` and `model.rs` to extract image metadata (`src`, `alt`, `width`, `height`), picking the first valid URL candidate (no complex `srcset` selectors), while storing the `alt` text for placeholders.

### Detailed Scope
- Extend `Block::Image` in `src/model.rs` to include `src: String`, `alt: String`, `width: Option<u32>`, `height: Option<u32>`.
- Update `parse.rs` `walk_element` to select the primary `src` attribute (or first candidate in `data-src`).
- Resolve relative image URLs against document base URL using `urlutil.rs`.

### Completion Checklist & Definition of Done
- [x] `model.rs` `Block::Image` contains full URL & metadata fields.
- [x] `parse.rs` extracts image URLs from `<img>` tags on `kalalawyer.com`.
- [x] Relative image URLs resolve cleanly against base URL.
- [x] `cargo check` and `cargo test` pass with zero errors.

### Error & Exception Handling
- If `src` is missing or unparseable, retain `Block::Image` with an empty `src` so the layout renderer displays `[ img: <alt> ]`.

### Internal Version Milestones

#### Version `v1.1.1`
- **Objective**: Update `Block::Image` in `model.rs` and update pattern matches in `layout.rs` and `snapshot.rs`.
- **Validation Criteria**: `cargo check` passes cleanly.

#### Version `v1.1.2`
- **Objective**: Extract first candidate `src` and `alt` text in `parse.rs` and resolve relative paths in `urlutil.rs`.
- **Validation Criteria**: `cargo run -- text https://kalalawyer.com` prints resolved image URLs alongside alt text.

---

## Stage 2: Rate-Limited Async Image Fetching Layer

### Primary Objective
Implement a rate-limited, asynchronous image binary fetcher in `src/fetch.rs` with a concurrency cap of max 4 simultaneous requests, strict timeouts, and size safeguards.

### Detailed Scope
- Add `fetch_image(url: &str)` in `src/fetch.rs` using Tokio async routines.
- Limit max simultaneous HTTP image downloads to 4 using `tokio::sync::Semaphore`.
- Enforce 10-second request timeout and 10MB payload size limit per image.
- Validate MIME types (`image/png`, `image/jpeg`, `image/webp`, `image/gif`).

### Error & Exception Handling
- Catch HTTP 4xx/5xx, timeouts, or oversized payloads; emit `FetchError::Failed` so the renderer gracefully displays the alt-text placeholder `[ img: <alt> (load failed) ]`.

### Internal Version Milestones

#### Version `v1.2.1`
- **Objective**: Add rate-limited `fetch_image` helper with Semaphore and timeout guards.
- **Validation Criteria**: Unit tests verify 4-download concurrency cap and size rejection.

---

## Stage 3: Multi-Format Image Decoder (PNG, JPEG, WebP, GIF Frame 0)

### Primary Objective
Integrate the Rust `image` crate (`image = "0.25"`) in `src/image_decoder.rs` to decode PNG, JPEG, WebP, and Animated GIF (extracting Frame 0 for zero-lag static rendering). SVG is explicitly excluded from initial scope.

### Detailed Scope
- Implement `decode_image(bytes: &[u8], mime: &str) -> Result<DynamicImage>`.
- For Animated GIFs, use `image::codecs::gif::GifDecoder` to decode the first frame (`Frame 0`) immediately.
- Return raw full-resolution `image::DynamicImage` objects before any terminal-specific scaling.

### Error & Exception Handling
- If byte decoding fails due to corrupted data, return `DecoderError::Corrupt` and trigger fallback to alt-text placeholder.

### Internal Version Milestones

#### Version `v1.3.1`
- **Objective**: Create `src/image_decoder.rs` with PNG, JPEG, WebP, and GIF Frame 0 decoding.
- **Validation Criteria**: Unit tests decode sample PNG, JPEG, WebP, and multi-frame GIF binaries into `DynamicImage`.

### Completion Checklist & Definition of Done
- [x] All 12 major development stages completed across Groups A, B, and C.
- [x] Universal TrueColor half-block, ASCII, and Braille renderers functional.
- [x] Session-wide capability detection cached once using `std::sync::OnceLock`.
- [x] Three-tier caching operational (Disk, Memory DynamicImage, Render cell lines).
- [x] Placeholder layout reflows dynamically when images finish downloading.
- [x] Viewport virtualization renders ONLY visible image lines.
- [x] Full-screen Modal Image Viewer with zoom, mode toggle, and save-to-disk functionality.
- [x] Target site `kalalawyer.com` advocate portraits & chamber photos render as full-color terminal graphics.
- [x] Benchmarks met (<300ms first img, <10ms cached img, 60 FPS scroll, <100MB RAM).
- [x] CLI flags `--no-images` and `--image-mode` supported.
- [x] Zero commits pushed to remote GitHub (100% local compliance).
- [x] Code compiles with 0 errors and passes all 17 unit tests.

---

## Stage 4: Three-Tier Caching Engine

### Primary Objective
Implement a three-tier caching system in `src/image_cache.rs` that prevents re-downloading raw bytes and avoids re-decoding images during terminal resize events.

### Detailed Scope
- **Tier 1 (Disk Cache)**: Persistent raw image bytes stored in `~/.cache/termbrowse/images/` using SHA-256 URL keys (max 50MB disk limit).
- **Tier 2 (In-Memory Full-Res Cache)**: Decoded, unscaled `image::DynamicImage` held in an LRU memory cache (up to 30 images). Serves as the source of truth for re-scaling during terminal resize events.
- **Tier 3 (Render Cache)**: Final terminal cell color/ANSI buffers scaled to current terminal dimensions. Invalidated automatically when terminal dimensions change.

### Error & Exception Handling
- If disk cache folder creation fails (e.g. read-only environment), log silently and operate on Tier 2 in-memory cache.

### Internal Version Milestones

#### Version `v1.4.1`
- **Objective**: Implement Three-Tier Cache structure in `src/image_cache.rs`.
- **Validation Criteria**: In-memory Tier 2 cache returns full-res `DynamicImage` in <10ms.

### Completion Checklist & Definition of Done
- [x] Universal TrueColor half-block renderer outputs crisp terminal images.
- [x] Pluggable ASCII and 8-dot Braille renderers functional.
- [x] Session-wide capability detection cached once using `std::sync::OnceLock`.
- [x] Placeholder layout reflows dynamically when images finish downloading.
- [x] Tokio `mpsc` channel triggers instant TUI layout reflow without UI freezing.
- [x] Viewport virtualization renders ONLY visible image lines.
- [x] Terminal resize events (`Event::Resize`) trigger instant re-scaling.
- [x] Code compiles with 0 errors and passes 17 unit tests.
- [x] All unit tests pass.

---

# Group B – Dynamic Layout Engine & Viewport Integration (Stages 5 – 9)

### Group Objective
Build a unified, event-driven layout and rendering engine where document blocks start as alt-text placeholders, asynchronously reflow upon image arrival, render via pluggable terminal engines (Half-Block, ASCII, Braille, Kitty), and virtualize visible viewport lines.

---

## Stage 5: Pluggable Renderers & Session Capability Caching

### Primary Objective
Implement universal ANSI 24-bit TrueColor Half-Block (`▀`), ASCII grayscale, Braille, and Kitty protocol renderers in `src/render_engine.rs`, caching terminal capabilities once per session using `std::sync::OnceLock`.

### Detailed Scope
- **Capability Detection**: Detect terminal capabilities (`TERM`, `TERM_PROGRAM`) once at startup using `std::sync::OnceLock<TerminalCaps>`.
- **Half-Block Engine**: Map 2 vertical pixels to 1 terminal cell using upper half-block `▀` (`\x1b[38;2;R1;G1;B1m\x1b[48;2;R2;G2;B2m▀\x1b[0m`).
- **Pluggable Renderers**: Implement ASCII grayscale density mapping, 8-dot Braille art, and Kitty graphics protocol.

### Internal Version Milestones

#### Version `v1.5.1`
- **Objective**: Create `src/render_engine.rs` with `std::sync::OnceLock` capability caching and Half-Block / ASCII / Braille / Kitty rendering methods.
- **Validation Criteria**: Render 2x2 test pattern cleanly into terminal cell spans.

---

## Stage 6: Placeholder-Driven Dynamic Document Layout Engine

### Primary Objective
Refactor `src/layout.rs` into a dynamic layout engine that begins with alt-text placeholders (`[ img: <alt> ]`), accepts decoded image dimensions asynchronously, and reflows document lines incrementally.

### Detailed Scope
- **Placeholder Layout**: Initially allocate cell bounding box for `Block::Image` based on explicit HTML attributes or fallback alt-text line `[ img: <alt> ]`.
- **Incremental Reflow**: Provide `reflow_image_block(doc_layout, image_index, dynamic_img, terminal_width)` to update lines dynamically upon image arrival.
- **Aspect Ratio Scaling**: Account for terminal font cell aspect ratio (~1:2 width-to-height ratio).

### Internal Version Milestones

#### Version `v1.6.1`
- **Objective**: Refactor `layout.rs` to support placeholder initialization and incremental block reflow.
- **Validation Criteria**: Layout correctly updates cell line count when image dimensions arrive.

---

## Stage 7: Async Image Event Channel & TUI Event Loop Integration

### Primary Objective
Connect background Tokio image fetchers with the TUI event loop in `src/tui_session.rs` via an `mpsc` channel, triggering incremental document reflow when images finish decoding.

### Detailed Scope
- **Tokio Channel**: `tokio::sync::mpsc::channel<ImageLoadEvent>` sending loaded `(image_index, DynamicImage)` to main TUI thread.
- **Non-Blocking UI**: TUI main loop stays 100% responsive at 60 FPS while images fetch in background.
- **Reflow Trigger**: Upon receiving `ImageLoadEvent`, update layout and trigger re-render of Tier 3 cell cache.

### Internal Version Milestones

#### Version `v1.7.1`
- **Objective**: Implement async image channel and background worker task in `src/tui_session.rs`.
- **Validation Criteria**: Images load in background and populate UI dynamically without freezing input.

---

## Stage 8: Viewport Virtualization Engine

### Primary Objective
Implement Viewport Virtualization in `src/tui_session.rs` so that only image cell lines currently inside `viewport_top..viewport_bottom` are rendered into active screen buffers.

### Detailed Scope
- Compute visible range based on current scroll offset and terminal window height.
- Skip color calculation, string formatting, and buffer copying for image lines outside the visible screen window.
- Enables smooth scrolling on pages with 100+ images.

### Internal Version Milestones

#### Version `v1.8.1`
- **Objective**: Implement viewport virtualization line slicer in `src/tui_session.rs`.
- **Validation Criteria**: CPU usage stays under 5% during rapid scrolling on image-heavy documents.

---

## Stage 9: Integrated Terminal Resize & Full Reflow Handler

### Primary Objective
Wire terminal resize events (`crossterm::event::Event::Resize(cols, rows)`) into the dynamic layout engine, using Tier 2 cached `DynamicImage` objects to reflow the document instantly without network re-fetching.

### Detailed Scope
- On `Event::Resize(w, h)`:
  1. Invalidate Tier 3 Render Cache.
  2. Recalculate document layout for new column width `w`.
  3. Re-scale Tier 2 `DynamicImage` objects to new cell bounds.
  4. Redraw virtualized viewport.

### Internal Version Milestones

#### Version `v1.9.1`
- **Objective**: Implement full layout reflow on `Event::Resize` using Tier 2 cache.
- **Validation Criteria**: Resizing terminal window updates image scales instantly with zero network requests.

---

# Group C – Interactive Features, Benchmarks & Release Polish (Stages 10 – 12)

### Group Objective
Add interactive image inspection modals, enforce strict benchmark performance targets on `kalalawyer.com`, finalize CLI flags, build unit tests, and update documentation.

---

## Stage 10: Interactive Image Viewer & Modal Overlay

### Primary Objective
Add keyboard navigation to focus images, open full-screen enlarged image modals (`Enter`), zoom (`+`/`-`), toggle render modes (`m`), and download raw images (`s`).

### Detailed Scope
- Add image focus selection in TUI document navigation.
- Full-screen modal overlay displaying image at maximum terminal resolution.
- Key `s` saves image file to `./downloads/`.

### Internal Version Milestones

#### Version `v1.10.1`
- **Objective**: Implement image focus highlight, full-screen modal viewer, zoom, and save commands.
- **Validation Criteria**: Full-screen modal displays enlarged image; `s` downloads raw image binary.

---

## Stage 11: Real-World Verification & Benchmark Targets

### Primary Objective
Verify image rendering on target site `https://kalalawyer.com`, measuring strict benchmark targets.

### Explicit Benchmark Targets
1. **Time to First Renderable Image**: `< 300 ms`
2. **Time to Render Cached Image**: `< 10 ms`
3. **Scroll Performance**: `0 dropped frames` (60 FPS)
4. **Memory Stability**: Bounded `< 100 MB` RAM with zero memory leaks across 100+ page navigations.

### Internal Version Milestones

#### Version `v1.11.1`
- **Objective**: Execute benchmark suite and verify target site `kalalawyer.com`.
- **Validation Criteria**: All 4 performance benchmark targets met 100%.

---

## Stage 12: Production Polish, CLI Flags & Final Audit

### Primary Objective
Finalize CLI flags (`--no-images`, `--image-mode`), build unit tests, update `README.md`, verify anti-push rule compliance, and prepare release `v0.3.0`.

### Detailed Scope
- Add `--no-images` and `--image-mode=halfblock|ascii|braille|kitty` in `src/main.rs`.
- Write unit test suite in `tests/image_tests.rs`.
- Update `README.md`.
- Verify forbidden `git push` rule respected.

### Internal Version Milestones

#### Version `v1.12.1`
- **Objective**: Complete CLI flags, unit tests, README update, and release candidate audit.
- **Validation Criteria**: `cargo test` passes 100%; `cargo build --release` succeeds cleanly.
