---
name: bevy-reflect
description: "Use when working with the `bevy_reflect` crate of bevy: Dynamically interact with Rust types."
---

# bevy_reflect

- Version: 0.20.0-dev (workspace member of `third_party/bevy`, vendored commit `4d79b30df4f7`)
- Source: `third_party/bevy/crates/bevy_reflect/` (manifest `third_party/bevy/crates/bevy_reflect/Cargo.toml`)
- Upstream: https://github.com/bevyengine/bevy/tree/main/crates/bevy_reflect
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Dynamically interact with Rust types

## README (`third_party/bevy/crates/bevy_reflect/README.md`)

# Bevy Reflect


This crate enables you to dynamically interact with Rust types:

* Derive the `Reflect` traits
* Interact with fields using their names (for named structs) or indices (for tuple structs)
* "Patch" your types with new values
* Look up nested fields using "path strings"
* Iterate over struct fields
* Automatically serialize and deserialize via Serde (without explicit serde impls)
* Trait "reflection"

## Features

### Derive the `Reflect` traits

```rust ignore
// this will automatically implement the `Reflect` trait and the `Struct` trait (because the type is a struct)
#[derive(Reflect)]
struct Foo {
    a: u32,
    b: Bar,
    c: Vec,
    d: Vec,
}

// this will automatically implement the `Reflect` trait and the `TupleStruct` trait (because the type is a tuple struct)
#[derive(Reflect)]
struct Bar(String);

#[derive(Reflect)]
struct Baz {
    value: f32,
}

// We will use this value to illustrate `bevy_reflect` features
let mut foo = Foo {
    a: 1,
    b: Bar("hello".to_string()),
    c: vec![1, 2],
    d: vec![Baz { value: 3.14 }],
};
```

### Interact with fields using their names

```rust ignore
assert_eq!(*foo.get_field::("a").unwrap(), 1);

*foo.get_field_mut::("a").unwrap() = 2;

assert_eq!(foo.a, 2);
```

### "Patch" your types with new values

```rust ignore
let mut dynamic_struct = DynamicStruct::default();
dynamic_struct.insert("a", 42u32);
dynamic_struct.insert("c", vec![3, 4, 5]);

foo.apply(&dynamic_struct);

assert_eq!(foo.a, 42);
assert_eq!(foo.c, vec![3, 4, 5]);
```

### Look up nested fields using "path strings"


## Cargo features (from the manifest)

- `default` (enables 5)
- `reflect_documentation` (enables 1): When enabled, allows documentation comments to be accessed via reflection
- `functions` (enables 1): Enables function reflection
- `debug` (enables 1): Enables features useful for debugging reflection
- `debug_stack` (enables 1): When enabled, keeps track of the current serialization/deserialization context for better error messages
- `annotations` (enables 1): Adds reflection support to `bevy_annotations` types and `CustomAttributes::value_range`.
- `glam` (enables 1): Adds reflection support to `glam` types.
- `hashbrown` (enables 1): Adds reflection support to `hashbrown` types.
- `indexmap` (enables 1): Adds reflection support to `indexmap` types.
- `petgraph` (enables 2): Adds reflection support to `petgraph` types.
- `smallvec` (enables 1): Adds reflection support to `smallvec` types.
- `uuid` (enables 1): Adds reflection support to `uuid` types.
- `wgpu-types` (enables 1): Adds reflection support to `wgpu-types` types.
- `std` (enables 8): Allows access to the `std` crate. Enabling this feature will prevent compilation on `no_std` targets, but provides access to certain additional features on supported platforms.
- `critical-section` (enables 1): `critical-section` provides the building blocks for synchronization primitives on all platforms, including `no_std`.
- `auto_register`: Enables automatic reflect registration. Does nothing by itself, must select `auto_register_inventory` or `auto_register_static` to make it work.
- `auto_register_inventory` (enables 3): Enables automatic reflect registration using inventory. Not supported on all platforms.
- `auto_register_static` (enables 2): Enable automatic reflect registration without inventory. This feature has precedence over `auto_register_inventory`. See `load_type_registrations` for more info.
- `web` (enables 2): Enables use of browser APIs. Note this is currently only applicable on `wasm32` architectures.

## Depends on (same ecosystem)

`bevy_annotations`, `bevy_platform`, `bevy_ptr`, `bevy_reflect_derive`, `bevy_utils`

## Public items by file

- `third_party/bevy/crates/bevy_reflect/src/array.rs`: trait Array, struct ArrayInfo, struct DynamicArray, struct ArrayIter, fn array_hash, fn array_apply, fn array_try_apply, fn array_partial_eq, fn array_partial_cmp, fn array_debug
- `third_party/bevy/crates/bevy_reflect/src/attributes.rs`: struct CustomAttributes, struct CustomAttributesBuilder
- `third_party/bevy/crates/bevy_reflect/src/convert.rs`: struct ReflectConvert
- `third_party/bevy/crates/bevy_reflect/src/enums/dynamic_enum.rs`: enum DynamicVariant, struct DynamicEnum
- `third_party/bevy/crates/bevy_reflect/src/enums/enum_trait.rs`: trait Enum, struct EnumInfo, struct VariantFieldIter, enum VariantField
- `third_party/bevy/crates/bevy_reflect/src/enums/helpers.rs`: fn enum_hash, fn enum_partial_eq, fn enum_partial_cmp, fn enum_debug
- `third_party/bevy/crates/bevy_reflect/src/enums/variants.rs`: enum VariantType, enum VariantInfoError, enum VariantInfo, struct StructVariantInfo, struct TupleVariantInfo, struct UnitVariantInfo
- `third_party/bevy/crates/bevy_reflect/src/error.rs`: enum ReflectCloneError
- `third_party/bevy/crates/bevy_reflect/src/fields.rs`: struct NamedField, struct UnnamedField, enum FieldId
- `third_party/bevy/crates/bevy_reflect/src/from_reflect.rs`: trait FromReflect, struct ReflectFromReflect
- `third_party/bevy/crates/bevy_reflect/src/func/args/arg.rs`: struct Arg, enum ArgValue
- `third_party/bevy/crates/bevy_reflect/src/func/args/count.rs`: struct ArgCount, struct ArgCountIter
- `third_party/bevy/crates/bevy_reflect/src/func/args/error.rs`: enum ArgError, struct ArgCountOutOfBoundsError
- `third_party/bevy/crates/bevy_reflect/src/func/args/from_arg.rs`: trait FromArg
- `third_party/bevy/crates/bevy_reflect/src/func/args/info.rs`: struct ArgInfo, enum ArgId
- `third_party/bevy/crates/bevy_reflect/src/func/args/list.rs`: struct ArgList
- `third_party/bevy/crates/bevy_reflect/src/func/args/ownership.rs`: enum Ownership, trait GetOwnership
- `third_party/bevy/crates/bevy_reflect/src/func/dynamic_function.rs`: struct DynamicFunction
- `third_party/bevy/crates/bevy_reflect/src/func/dynamic_function_mut.rs`: struct DynamicFunctionMut
- `third_party/bevy/crates/bevy_reflect/src/func/error.rs`: enum FunctionError, type FunctionResult, enum FunctionOverloadError, enum FunctionRegistrationError
- `third_party/bevy/crates/bevy_reflect/src/func/function.rs`: trait Function
- `third_party/bevy/crates/bevy_reflect/src/func/info.rs`: struct FunctionInfo, struct SignatureInfo, struct ReturnInfo, struct PrettyPrintFunctionInfo, struct PrettyPrintSignatureInfo, trait TypedFunction
- `third_party/bevy/crates/bevy_reflect/src/func/into_function.rs`: trait IntoFunction
- `third_party/bevy/crates/bevy_reflect/src/func/into_function_mut.rs`: trait IntoFunctionMut
- `third_party/bevy/crates/bevy_reflect/src/func/mod.rs`: mod args, mod signature
- `third_party/bevy/crates/bevy_reflect/src/func/reflect_fn.rs`: trait ReflectFn
- `third_party/bevy/crates/bevy_reflect/src/func/reflect_fn_mut.rs`: trait ReflectFnMut
- `third_party/bevy/crates/bevy_reflect/src/func/registry.rs`: struct FunctionRegistry, struct FunctionRegistryArc
- `third_party/bevy/crates/bevy_reflect/src/func/return_type.rs`: enum Return, trait IntoReturn
- `third_party/bevy/crates/bevy_reflect/src/func/signature.rs`: struct Signature, struct ArgumentSignature
- `third_party/bevy/crates/bevy_reflect/src/generics.rs`: struct Generics, enum GenericInfo, struct TypeParamInfo, struct ConstParamInfo
- `third_party/bevy/crates/bevy_reflect/src/info/error.rs`: enum TypeInfoError
- `third_party/bevy/crates/bevy_reflect/src/info/opaque.rs`: struct OpaqueInfo
- `third_party/bevy/crates/bevy_reflect/src/info/type_info.rs`: enum TypeInfo
- `third_party/bevy/crates/bevy_reflect/src/info/typed.rs`: trait Typed, trait DynamicTyped, trait MaybeTyped
- `third_party/bevy/crates/bevy_reflect/src/is.rs`: trait Is
- `third_party/bevy/crates/bevy_reflect/src/kind.rs`: enum ReflectKind, struct ReflectKindMismatchError, enum ReflectRef, enum ReflectMut, enum ReflectOwned
- `third_party/bevy/crates/bevy_reflect/src/lib.rs`: mod array, mod display, mod func, mod list, mod map, mod set, mod structs, mod tuple, mod tuple_struct, mod attributes, mod convert, mod enums, mod serde, mod std_traits, mod ty, mod utility, mod prelude, mod __macro_exports
- `third_party/bevy/crates/bevy_reflect/src/list.rs`: trait List, struct ListInfo, struct DynamicList, struct ListIter, fn list_hash, fn list_apply, fn list_try_apply, fn list_partial_eq, fn list_partial_cmp, fn list_debug
- `third_party/bevy/crates/bevy_reflect/src/map.rs`: trait Map, struct MapInfo, struct DynamicMap, fn map_partial_eq, fn map_partial_cmp, fn map_debug, fn map_apply, fn map_try_apply, macro hash_error!
- (26 more files)

## Examples in the source tree

- `third_party/bevy/crates/bevy_reflect/examples/reflect_docs.rs`

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
