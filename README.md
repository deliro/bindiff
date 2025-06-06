# bindiff

`bindiff` is a simple CLI tool for creating and applying binary patches.

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
