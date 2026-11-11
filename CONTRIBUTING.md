# Contributing

## Building and testing

What CI runs, besides clippy on each feature alone and a check of the package's contents:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo build --lib --target wasm32-unknown-unknown
cargo +1.85 check --all-targets --all-features --locked   # the oldest Rust supported
```

Most tests read real films. They live outside this repository: set `FILMSHELL_TEST_FILMS` to a
folder with one folder per film, each holding the film's chunks (`filmChunk0`, `filmChunk1`, ...,
compressed as downloaded or inflated as `filmChunk{N}_dec`) and its `film-metadata.json`. Without
the variable the tests look in `films/` here (ignored by git), and with no films they check
nothing. With the variable set, a missing film fails the tests.

```sh
FILMSHELL_TEST_FILMS=/path/to/films cargo test --release --all-features
```

The tests read eight films at a time and keep every film read: on 95 films they take about two
minutes and 6 GB of memory.

CI runs them against a private set of films, since films carry every player's gamertag and XUID.
Pull requests from forks run everything else; a maintainer runs the films before merging. With the
films, CI also runs every example on every film:

```sh
cargo build --release --all-features --examples
sh .github/examples-on-films.sh /path/to/films
```

## How the format is known

The game's own code is the source of truth for the format, and films verify it. A comment that
states a layout says how it is known: read from the game's code, measured on films, or inferred.

## Changes

- The model at the crate root follows semantic versioning. The raw layer, `filmshell::format`, may
  change in any minor release.
- Note user-visible changes in `CHANGELOG.md` under Unreleased.
- The tables in `src/game/*_data.rs` are generated from the game's files; don't edit them by hand.

## Releasing

1. Set the version in `Cargo.toml`, move the changelog's Unreleased notes under it, point the
   README's install line at the version (`filmshell = "x.y"`) rather than the repository, and
   merge.
2. Tag the merge `x.y.z`, without a `v`, and push the tag. The Release workflow checks the version,
   runs the tests and publishes to crates.io.

Publishing uses crates.io trusted publishing. The first version has to be published by hand
with `cargo publish` and a token; after that, add this repository's `release.yml` workflow and its
`release` environment as the crate's trusted publisher on crates.io.
