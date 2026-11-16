---
name: bevy-ptr
description: "Use when working with the `bevy_ptr` crate of bevy: Utilities for working with untyped pointers in a more safe way."
---

# bevy_ptr

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_ptr/` (manifest `third_party/bevy/crates/bevy_ptr/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_ptr
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Utilities for working with untyped pointers in a more safe way

## README (`third_party/bevy/crates/bevy_ptr/README.md`)

# Bevy Pointer


Pointers in computer programming are objects that store a memory address. They're a fundamental building block for constructing more
complex data structures.

They're also *the* definitive source of memory safety bugs: you can dereference an invalid (null) pointer, access a pointer after the underlying
memory has been freed, and even ignore type safety and misread or mutate the underlying memory improperly.

Rust is a programming language that heavily relies on its types to enforce correctness, and by proxy, memory safety. As a result,
Rust has an entire zoo of types for working with pointers, and a graph of safe and unsafe conversions that make working with them safer.

`bevy_ptr` is a crate that attempts to bridge the gap between the full blown unsafety of `*mut ()` and the safe `&'a T`, allowing users
to choose what invariants to uphold for their pointer, with the intent to enable building progressively safer abstractions.

## How to Build a Borrow (From Scratch)

Correctly and safety converting a pointer into a valid borrow is at the core of all `unsafe` code in Rust. Looking at the documentation for
[`(*const T)::as_ref`], a pointer must satisfy *all* of the following conditions:

* The pointer must be properly aligned.
* The pointer cannot be null, even for zero sized types.
* The pointer must be within bounds of a valid allocated object (on the stack or the heap).
* The pointer must point to an initialized instance of `T`.
* The newly assigned lifetime should be valid for the value that the pointer is targeting.
* The code must enforce Rust's aliasing rules. Only one mutable borrow or arbitrarily many read-only borrows may exist to a value at any given moment
  in time, and converting from `&T` to `&mut T` is never allowed.

Note these rules aren't final and are still in flux as the Rust Project hashes out what exactly are the pointer aliasing rules, but the expectation is that the
final set of constraints are going to be a superset of this list, not a subset.

This list already is non-trivial to satisfy in isolation. Thankfully, the Rust core/standard library provides a progressive list of pointer types that help
build these safety guarantees...

## Standard Pointers

|Pointer Type       |Lifetime'ed|Mutable|Strongly Typed|Aligned|Not Null|Forbids Aliasing|Forbids Arithmetic|
|-------------------|-----------|-------|--------------|-------|--------|----------------|------------------|
|`Box`           |Owned      |Yes    |Yes           |Yes    |Yes     |Yes             |Yes               |
|`&'a mut T`        |Yes        |Yes    |Yes           |Yes    |Yes     |Yes             |Yes               |
|`&'a T`            |Yes        |No     |Yes           |Yes    |Yes     |No              |Yes               |
|`&'a UnsafeCell`|Yes        |Maybe  |Yes           |Yes    |Yes     |Yes             |Yes               |
|`NonNull`       |No         |Yes    |Yes           |No     |Yes     |No              |No                |
|`*const T`         |No         |No     |Yes           |No     |No      |No              |No                |
|`*mut T`           |No         |Yes    |Yes           |No     |No      |No              |No                |
|`*const ()`        |No         |No     |No            |No     |No      |No              |No                |
|`*mut ()`          |No         |Yes    |No            |No     |No      |No              |No                |

`&T`, `&mut T`, and `Box` are by far the most common pointer types that Rust developers will see. They're the only ones in this list that are entirely usable
without the use of `unsafe`.

`&UnsafeCell` is the first step away from safety. `UnsafeCell` is the *only* way to get a mutable borrow from an immutable one in the language, so it's the
base primitive for all interior mutability in the language: `Cell`, `RefCell`, `Mutex`, `RwLock`, etc. are all built on top of
`UnsafeCell`. To safety convert `&UnsafeCell` into a `&T` or `&mut T`, the caller must guarantee that all simultaneous access follow Rust's aliasing rules.

`NonNull` takes quite a step down from the aforementioned types. In addition to allowing aliasing, it's the first pointer type on this list to drop both
lifetimes and the alignment guarantees of borrows. Its only guarantees are that the pointer is not null and that it points to a valid instance
of type `T`. If you've ever worked with C++, `NonNull` is very close to a C++ reference (`T&`).

`*const T` and `*mut T` are what most developers with a background in C or C++ would consider pointers.

`*const ()` is the bottom of this list. They're the Rust equivalent to C's `void*`.  Note that Rust doesn't formally have a concept of type that holds an arbitrary
untyped memory address. Pointing at the unit type (or some other zero-sized type) just happens to be the convention. The only way to reasonably use them is to
cast back to a typed pointer. They show up occasionally when dealing with FFI and the rare occasion where dynamic dispatch is required, but a trait is too
constraining of an interface to work with. A great example of this are the [RawWaker] APIs, where a singular trait (or set of traits) may be insufficient to capture
all usage patterns. `*mut ()` should only be used to carry the mutability of the target, as there is no way to mutate an unknown type.


## Available in Nightly


## Public items by file

- `third_party/bevy/crates/bevy_ptr/src/lib.rs`: struct Aligned, struct Unaligned, trait IsAligned, struct ConstNonNull, struct Ptr, struct PtrMut, struct OwningPtr, struct MovingPtr, struct ThinSlicePtr, trait UnsafeCellDeref, macro move_as_ptr!, macro get_pattern!, macro deconstruct_moving_ptr!

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
