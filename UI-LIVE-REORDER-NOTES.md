# UI refinement: live image-page reordering

## Changes
- Image-to-PDF card sorting renders a projected order while a card is dragged.
- The grabbed page remains visually attached to the pointer, while a separately painted insertion gap stays in the grid.
- Neighboring cards reflow as the insertion point changes, including when moving between rows.
- The real file list is changed only on pointer release, avoiding repeated list mutations and reorder jitter.
- Escape cancels an active reorder without changing the page sequence.
- The insertion gap uses the dragged card's measured dimensions and is highlighted with an accent outline and "Drop here" label.
- Added a helper and unit test for moving an item to its final zero-based index. Existing arrow-button insertion semantics remain unchanged.

## Verification
- Archive integrity checks, delimiter checks, package-source matching, and an exhaustive source/target index mapping check for list lengths 1–7 passed.
- Rust compilation and `cargo test` were not run in this build environment because `cargo` and `rustc` are unavailable.

## Update instructions
Replace `src/main.rs` with the included file, then run:

```sh
cargo check
cargo test
cargo run --release
```

Do not run `cargo clean`; the existing Cargo build cache should be reused.
