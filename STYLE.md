# nie: low-level style and agent method

Read `MAP.tsv` first (`just map` regenerates it; `just map --check` fails CI
when stale). `.rgignore` and `.fdignore` hide `vendor/`, `third_party/`, `target/`, lockfiles and generated
bindings from default searches; use `rg --no-ignore` to audit them.

## Low-level Rust contract

- Flat data: plain structs, `#[repr(C)]` where a layout crosses the C ABI, `&[u8]` and `&mut [u8]` for bytes,
  indices instead of references between records, no `Rc`/`RefCell` graphs.
- No abstraction without a second user: a trait needs two implementors in the tree, a generic needs two
  instantiations, a builder needs more than five fields. Otherwise write the function.
- Ownership table: a type either owns its bytes (`Vec<u8>`, `Box<[u8]>`) or borrows them for one call
  (`&[u8]`); a struct that stores a borrow carries one lifetime and is built and dropped in one function.
  Anything longer-lived owns, or holds an `Arc` created at the process edge.
- Errors are values: `Result` with one error enum per crate, no panics on input bytes, no `unwrap` outside
  tests, `unsafe` only behind a `// SAFETY:` comment naming the invariant and a test that breaks it.
- No stubs: an unfinished function is deleted, not stubbed; a workaround that needs a long comment is rejected.
- Behaviour copied from game code is proven by a differential test against recorded outputs, and debug-only
  asserts never carry side effects.

## Agent method (after https://bun.com/blog/bun-in-rust)

1. A guide first (this file plus the task's mapping table), then a pilot on three files before scaling.
2. One task = one implementer, two adversarial reviewers that see only the diff, one fixer.
3. Agents commit their own files by path; none runs `git stash` or `git reset`; none builds on the PC. Builds
   and tests run on fr1 in tmux in the agent's own scratch directory, deleted at the end.
4. Progress is gated by tests that exist independently of the change; nothing is skipped or deleted to pass.
5. A bad result is fixed in the workflow prompt, not by hand-editing the output.
