# Project structure

```
src/
  lib.rs                   re-exports: CrossrefClient, Config, CrossrefError,
                           Result, WorkMeta, BibRecord, SearchQuery,
                           SearchResult, KeyStyle
  client.rs                CrossrefClient (Crossref + Unpaywall HTTP)
  config.rs                Config struct, load priority, first-run guidance
  cache.rs                 DiskCache (JSON files, TTL-based expiry)
  models.rs                WorkMeta, BibRecord, SearchQuery, SearchResult
                           (all derive derive_builder + serde)
  utils.rs                 citation key generation, DOI normalisation,
                           key conflict resolution
  bibtex.rs                work_to_bib_record*, records_to_bibtex,
                           append_to_bib_file (dedup + conflict resolution)
  error.rs                 CrossrefError enum, Result type alias
  bin/
    cli.rs                 clap CLI, all subcommand handlers
    nu_plugin_crossref.rs  nu-plugin 0.111 plugin

tests/
  client_tests.rs          async wiremock tests for all CrossrefClient methods
  bibtex_tests.rs          BibTeX conversion, key styles, append dedup
  cache_tests.rs           DiskCache set/get/TTL/clear
  config_tests.rs          Config priority, env overrides, first-run
```

## Short flags reference

### Global (both CLI and subcommands)

| Flag        | Short |
| ----------- | ----- |
| `--config`  | `-c`  |
| `--email`   | `-e`  |
| `--format`  | `-f`  |
| `--verbose` | `-v`  |

### `fetch-bib`

| Flag          | Short |
| ------------- | ----- |
| `--append`    | `-a`  |
| `--key-style` | `-k`  |

### `search`

| Flag            | Short |
| --------------- | ----- |
| `--title`       | `-t`  |
| `--author`      | `-a`  |
| `--year-from`   | `-F`  |
| `--year-to`     | `-T`  |
| `--type`        | `-y`  |
| `--open-access` | `-A`  |
| `--rows`        | `-n`  |
| `--sort`        | `-s`  |

### `pdf`

| Flag       | Short |
| ---------- | ----- |
| `--output` | `-o`  |

Nu plugin `SearchCommand` carries the same short flags (`-F`, `-T`, `-y`, `-A`; `-e`, `-t`, `-a`, `-n`, `-s` already existed).

## Nushell plugin version selection

The `nu_plugin_crossref` binary can be compiled against two nu-plugin versions:

| Feature flag        | Nushell target | Build command                                          |
| ------------------- | -------------- | ------------------------------------------------------ |
| `nu-v111` (default) | nushell 0.111  | `cargo build`                                          |
| `nu-v110`           | nushell 0.110  | `cargo build --no-default-features --features nu-v110` |

**How it works:**

- Both `nu-plugin 0.111` and `nu-plugin 0.110` are optional deps with distinct Cargo names (`nu-plugin-v110`, `nu-protocol-v110` for 0.110).
- `src/bin/nu_plugin_crossref.rs` has `#[cfg(feature = "nu-v110")] extern crate nu_plugin_v110 as nu_plugin;` at the top; the rest of the file is version-agnostic.
- `nu-plugin-core 0.110.0` (a transitive dep) had a broken `interprocess` import (`local_socket::traits::ListenerNonblockingMode` was moved to `local_socket::ListenerNonblockingMode` in interprocess 2.3.x). A local patch at `patches/nu-plugin-core-v110/` applies the same one-line fix that 0.111.0 contains. Cargo uses the patch only for the 0.110.0 dep resolution.

**Key invariant:** Do not remove `[patch.crates-io]` from `Cargo.toml` — it's required for `nu-v110` builds. The patch is in `patches/nu-plugin-core-v110/` and must not be deleted.
