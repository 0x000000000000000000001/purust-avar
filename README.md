# AVar

[![CI](https://github.com/purescript-contrib/purescript-avar/workflows/CI/badge.svg?branch=main)](https://github.com/purescript-contrib/purescript-avar/actions?query=workflow%3ACI+branch%3Amain)
[![Release](https://img.shields.io/github/release/purescript-contrib/purescript-avar.svg)](https://github.com/purescript-contrib/purescript-avar/releases)
[![Pursuit](https://pursuit.purescript.org/packages/purescript-avar/badge)](https://pursuit.purescript.org/packages/purescript-avar)
[![Maintainer: garyb](https://img.shields.io/badge/maintainer-garyb-teal.svg)](https://github.com/garyb)

Low-level interface for asynchronous variables.

## Installation

Install `avar` with [Spago](https://github.com/purescript/spago):

```sh
spago install avar
```

## Quick start

The quick start hasn't been written yet (contributions are welcome!). The quick start covers a common, minimal use case for the library, whereas longer examples and tutorials are kept in the [docs directory](./docs).

## Documentation

### Rust tests

Run `bin/test -c` to rebuild the sibling `purust` compiler, clear this package's
caches, generate fresh TAST and Rust with `--threaded`, and run the tests.
`bin/test` skips the compiler rebuild. Both builds use the compiler's local
Spago dependency. The sibling TAST-enabled PureScript fork is selected
automatically; `PURS=/path/to/purs` overrides it.

The suite preserves all 16 `gopurs-avar` tests unchanged. Eight additional
PureScript checks cover status transitions, FIFO ordering, read broadcasts,
reentrant callbacks, cancellation, kill, callback errors and effect replay.
Two Rust tests check that cancellation releases captured callbacks and queued
values, and that four producer threads and four consumer threads deliver
1,000 distinct values exactly once. A barrier and recorded thread IDs ensure
these are eight real worker threads; callbacks also access the same AVar
again to check that its mutex is not held during notification.

The Rust implementation stores state and FIFO queues behind a mutex.
Cancellation removes the queued entry immediately. Reads do not consume the
value; a selected take reserves it before notifications, preventing another
thread from taking the same value. Notifications run in read, take, put
order. Callback exceptions are rethrown after draining the pending work,
leaving the AVar usable.

### API

`avar` documentation is stored in a few places:

1. Module documentation is [published on Pursuit](https://pursuit.purescript.org/packages/purescript-avar).
2. Usage examples can be found in [the test suite](./test).

If you get stuck, there are several ways to get help:

- [Open an issue](https://github.com/purescript-contrib/purescript-avar/issues) if you have encountered a bug or problem.
- Ask general questions on the [PureScript Discourse](https://discourse.purescript.org) forum or the [PureScript Discord](https://purescript.org/chat) chat.

## Contributing

You can contribute to `avar` in several ways:

1. If you encounter a problem or have a question, please [open an issue](https://github.com/purescript-contrib/purescript-avar/issues). We'll do our best to work with you to resolve or answer it.

2. If you would like to contribute code, tests, or documentation, please [read the contributor guide](./CONTRIBUTING.md). It's a short, helpful introduction to contributing to this library, including development instructions.

3. If you have written a library, tutorial, guide, or other resource based on this package, please share it on the [PureScript Discourse](https://discourse.purescript.org)! Writing libraries and learning resources are a great way to help this library succeed.
