# Mates

A very simple addressbook program to be used together with
[vdirsyncer](https://github.com/untitaker/vdirsyncer).


## Installation

You need to have Rust nightly and Cargo installed. Just run ``cargo build`` to
get a binary which only depends on a working ``grep`` command in your path.


## Usage

Run the binary with ``--help`` to list all environment variables that can be
used for configuration. ``MATES_INDEX`` and ``MATES_DIR`` must be set.

- Configure mutt to use mates:

  ```
  set query_command= "mates --mutt-search '%s'"
  bind editor <Tab> complete-query
  bind editor ^T    complete
  ```

## License

mates is released under the public domain.
