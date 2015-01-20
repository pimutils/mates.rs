# Mates

A very simple addressbook program, operating on a directory of ``.vcf`` files,
see [vdir](http://vdirsyncer.readthedocs.org/en/stable/vdir.html). It was
written as an experiment to learn Rust, and might contain ugly code, it should
work well and fast though.

## Installation

You need to have Rust nightly and Cargo installed.

1. Clone this repository, `cd` to it.
2. Run `cargo build --release`. You could omit the release-flag, but then mates
   would run awfully slow.
3. The resulting binary is in `./target/release/` (`./target/` if you built
   without the release-flag), and depends on `glibc`, and `grep` in your
   `PATH`.


## Usage

Run the binary with ``--help`` to list all environment variables that can be
used for configuration. ``MATES_INDEX`` and ``MATES_DIR`` must be set.

- Set ``MATES_INDEX`` to a location where the program can generate an index
  file for performance purposes.

- Set ``MATES_DIR`` to your directory of ``.vcf``-files.

The other environment variables are:

- ``MATES_GREP``, override the grep binary to use. Default to ``grep``.
- ``MATES_EDITOR``, override the vCard editor to use. Default to ``EDITOR``.

``mates index`` must be called periodically. Even when using mates' own
commands, the index will be not updated automatically, as this would impact UI
responsiveness massively.


## Integration

### Mutt

    set query_command= "mates mutt-query '%s'"
    bind editor <Tab> complete-query
    bind editor ^T    complete
    macro index,pager A "<pipe-message>mates add | xargs mates edit<enter>" "add the sender address"

### Selecta (and similar)

[selecta](https://github.com/garybernhardt/selecta) is a fuzzy text selector
that can be used instead of grep to search for contacts.

    m() {
        mutt "$(MATES_GREP=selecta mates email-query)"
    }

### Synchronization with CardDAV (and others)

[Vdirsyncer](http://vdirsyncer.readthedocs.org/) can be used to synchronize to
a CardDAV server, using the `filesystem` storage. If you don't need that, using
any decent file synchronization will work too.

## License

mates is released under the public domain.
