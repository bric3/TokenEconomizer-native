# Native test conversion

The pinned original pytest collection contains 1,591 cases across 67 modules.
The original files remain unchanged under `reference/upstream`; converted tests
call the Rust library directly. Parameterised cases use Rust loops with the
original values. Test function names retain the original names without `test_`.

The first batch converts all assertions and parameter values in these modules:

| Original module                    | Rust tests | Original cases |
| ---------------------------------- | ---------: | -------------: |
| test_cache_write_1h.py             |         17 |             33 |
| test_config.py                     |          5 |              5 |
| test_config_integer_thresholds.py  |          7 |             50 |
| test_contribution_finite_config.py |          1 |              4 |
| test_ledger.py                     |          5 |              5 |
| test_load_calls_malformed.py       |          7 |             19 |
| test_observe_label_failed.py       |          1 |              1 |
| test_pricing_new_models.py         |          4 |             24 |
| test_repo_key_env.py               |          2 |              2 |
| test_store_read_lines.py           |          3 |              3 |
| test_windows_identity.py           |          3 |              3 |
| Total                              |         55 |            149 |

Each module is in `tests/` with the same basename and `.rs`, omitting `test_`.
The source path is recorded at the top of each converted file.
The first batch's 55 converted tests run alongside 45 existing native regression
tests.

The configuration tests retain integer-type checks and verify that rejected
updates preserve saved bytes. The cache fixtures retain duplicate Claude message
blocks, Pi parent links and usage splits. Pricing comparisons retain pytest's
relative and absolute tolerances. Windows-style label tests create actual files
and exercise all original state writers; they run on macOS and do not establish
native Windows execution. The label-failure test calls Rust directly; Python's
deprecation-warning filter is unnecessary for that implementation.

`tests/support/mod.rs` supplies isolated temporary directories for filesystem
fixtures. It removes successful fixtures and retains failed fixtures with their
path in the test output. No Python runtime or transport is used by these tests.

The second batch adds the trace seam, atomic-store retries and vendor costs:

| Original module                       | Rust tests | Original cases |
| ------------------------------------- | ---------: | -------------: |
| test_trace.py                         |         26 |             26 |
| test_store_windows_replace.py         |          8 |             10 |
| experiments/tests/test_vendor_cost.py |         13 |             13 |
| Batch total                           |         47 |             49 |

The trace fixtures retain request-blob duplicates, all catalog sources, prompt
types, Pi branch links and non-object JSON lines. The six attempt-result tests
now inspect the real aggregator's single-row counts; the original tests checked
a copied calculation helper. Store retry tests inject permission failures into
the same reader and atomic writer that `Store` calls. They check retry counts,
propagated errors, retained state and temporary-file cleanup. The real held-file
test uses Windows reader-sharing flags and a delayed close. That test is
conditional on Windows and has not been compiled or run on macOS.

The third batch adds adapters, index protection and the learned-fact lifecycle:

| Original module                             | Rust tests | Original cases |
| ------------------------------------------- | ---------: | -------------: |
| experiments/tests/test_adapters.py          |          7 |              7 |
| experiments/tests/test_aggregate_index.py   |          5 |              5 |
| experiments/tests/test_envfacts_profiles.py |          7 |              7 |
| experiments/tests/test_facade.py            |          3 |              3 |
| test_injection.py                           |          4 |              4 |
| test_distiller_payload_types.py             |          6 |             32 |
| Batch total                                 |         32 |             58 |

Adapter protocol conformance is checked against Rust's `DomainAdapter` trait;
non-null return contracts use Rust's concrete return types. Index tests write
actual row trees and check both CSVs, including preserving the canonical index
after a refused subset rewrite. The CSV reader accepts only these fixtures'
unquoted fields and asserts that restriction before reading them.

The distiller and facade tests keep proposer payloads and token charges in a
recording runner. No model process is started. The composed profile keeps the
six-task learning sequence and the additional telemetry task. The Codex facade
keeps the full ten-task sequence that reaches its paid proposer cadence. Cadence
is supplied through `ProposeContext`, keeping each test's configuration local.
The injection-profile constants are checked through the native arm registry,
which owns those configurations. Shared Claude and Codex transcript writers
retain the original fixtures' call IDs, optional working directories, item
defaults and event wrappers.

Run the native suite and the conversion inventory from the checkout root:

```sh
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
jbang validation/TestConversionInventory.java
```

The inventory collects the pinned original test IDs and matches their function
names to the Rust files' source annotations. It reports remaining modules and
functions. Name matching does not verify assertions, parameter values, fixture
behaviour or execution; those require source review and native test runs.
The inventory's Python dependency is limited to collecting the archived suite.

Conversion remains in progress: 20 of 67 pytest modules have native equivalents,
covering 256 original cases, including the conditional Windows case. The other
1,335 cases and standalone executable smoke tests
still need native equivalents. The broader parity and platform audits in
[the migration record](migration.md) remain open.
