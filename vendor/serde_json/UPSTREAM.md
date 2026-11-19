serde_json 1.0.151

Source: https://github.com/serde-rs/json/tree/de8500740cdcabffb9734f503e4889def823cf10
Published crate: https://static.crates.io/crates/serde_json/serde_json-1.0.151.crate
Crate SHA-256: c841b55ecdae098c80dcae9cf767f6f8a0c2cdb3416bbef72181df4d0fe73f14
Licence: MIT OR Apache-2.0. Both licence texts are preserved in this directory.

Local changes, 2026-11-19:

- Add the python_numbers feature, which enables arbitrary_precision and
  float_roundtrip. It retains compact storage for ordinary integers and floats,
  and stores integer digits beyond the signed/unsigned 64-bit ranges as text.
- Parse and emit NaN, Infinity and -Infinity. Float overflow produces infinity.
  Parse integer -0 as integer zero. Float spellings use the binary float value.
- Preserve wide integers through Value conversion and 128-bit construction.
  JSON NaN values compare equal; primitive float comparisons follow IEEE rules.
- Identify internal Number and RawValue markers using byte visitors. Literal
  object keys with those names remain ordinary string keys.
- Replace deprecated usize::max_value in lexical/math.rs with usize::MAX for
  current Rust toolchains. Standard features retain their upstream behaviour.

Changed upstream files: Cargo.toml, Cargo.toml.orig, src/de.rs, src/number.rs,
src/raw.rs, src/ser.rs, src/value/de.rs and src/lexical/math.rs.
Run jbang scripts/VerifySerdeJson.java to verify the published archive and this
change list. Run cargo test --test json_numbers for the compatibility contract.
Upstream tests remain unchanged and run with python_numbers disabled.

The feature does not impose Python's configurable integer digit limit. It does
not extend UTF-16 surrogate or nesting support, or arithmetic in callers that
still use bounded integer types.
