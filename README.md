# Mates

A very simple addressbook program to be used together with
[vdirsyncer](https://github.com/untitaker/vdirsyncer). It was written as an
experiment to learn Rust, and it might contain hideous code. While it works
fine for me (and can't do much damage due to its limited featureset), you're
probably better off using one of the [alternatives listed in vdirsyncer's
documentation](http://vdirsyncer.readthedocs.org/en/latest/supported.html#client-applications).


## Installation

You need to have Rust nightly and Cargo installed. Just run ``cargo build`` to
get a binary which only depends on a working ``grep`` command in your path.


## Usage

Run the binary with ``--help`` to list all environment variables that can be
used for configuration. ``MATES_INDEX`` and ``MATES_DIR`` must be set.

Before first usage and after each sync you need to recreate the index with
``mates -i``.

Features:

- Configure mutt to use mates:

  ```
  set query_command= "mates --mutt-search '%s'"
  bind editor <Tab> complete-query
  bind editor ^T    complete
  ```

## License

mates is released under the public domain.
