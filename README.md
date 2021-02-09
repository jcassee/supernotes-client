# Supernotes Client

This is a simple command line [Supernotes](https://supernotes.app/) tool
written in Rust. Currently it only creates new cards.


## Usage

Either set environment variables `SN_USERNAME` and `SN_PASSWORD` to your
Supernotes username and password, or use the `-u` and `-p` options.


### Creating a new card

Use the `create` command to create a new card. Specify the card title and the
file that contains the card body.

    sn create "Meeting notes" notes.md

If the file is omitted, the body is read from the standard input.

    sn create "Groceries" <<.
    - Milk
    - Bread
    .

## Build

The Cargo manifest is set up to optimize and strip the release binary, so you
need to use the nightly toolchain.

    cargo +nightly -Z unstable-options build --release
