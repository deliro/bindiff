# bindiff

`bindiff` is a simple CLI tool for creating and applying binary patches.

## Motivation
When two binary files are similar, rewriting or transmitting the entire new version can be inefficient. This tool reduces the cost by generating a compact binary patch that represents only the difference. The new file can then be reconstructed by applying the patch to the original.

## Patch Format
A patch is a sequence of opcodes:

* `Copy (0x00, offset, len)`: Copy len bytes from offset in the original file.
* `Add (0x01, len, bytes)`: Add len new bytes directly.

## Patch Generation
The original file is indexed using a sliding window (default: 8 bytes).

The new file is scanned with the same window.

If a match is found in the original file, the longest matching sequence is emitted as a Copy.

Otherwise, unmatched bytes are grouped into an Add.

## Patch Application
To apply a patch:

1. Start with an empty output buffer.
2. Iterate over the patch:
    1. Copy: append bytes from the original file.
    2. Add: append new bytes from the patch.

This reconstructs the new version of the file.

## When to Use
This tool works best when the two binary files are mostly similar — for example, when only small regions have changed.

## Commands

### diff

Creates a binary patch from two files and writes the patch to stdout.

```sh
bindiff diff old_file.bin new_file.bin > patch.bin
```

### patch

Applies a binary patch to a file and writes the result to stdout.

```sh
bindiff patch old_file.bin patch.bin > new_file.bin
```

## Install from sources

```sh
cargo install --git https://github.com/tochka-public/bindiff
```

## Build

```sh
cargo build --release
```

## Run

```sh
./target/release/bindiff diff a.bin b.bin > patch
./target/release/bindiff patch a.bin patch > b.bin
```
