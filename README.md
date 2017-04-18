# Mates

[![Build status](https://travis-ci.org/pimutils/mates.rs.svg?branch=master)](https://travis-ci.org/pimutils/mates.rs)

A commandline addressbook. The main goals are:

- **Few features, high extensibility**

  - Mates operates on a directory of
    [vCard](https://tools.ietf.org/html/rfc6350) files, a standardized file
    format for contacts. Because of this, another program can be used for
    synchronization with CardDAV-servers (see below).

  - Mates doesn't come with a contact editor. It relies on other programs to
    fulfill this task (which can be configured with ``MATES_EDITOR``), by
    default it will open your text editor.

- **UI responsiveness** For completing email addresses in mutt, mates maintains
  a simple textfile with only a few fields from the vCard file, on which it
  calls ``grep``. The textfile looks like this:

      work@example.com\tExample Man\t/home/user/.contacts/exampleman.vcf
      home@example.com\tExample Man\t/home/user/.contacts/exampleman.vcf


## Installation

For a manual installation, you need to have [Rust](http://www.rust-lang.org/)
and [Cargo](https://crates.io/) installed. Both 1.0 and the nightlies should
work.

1. `cargo install mates` (or `cargo install --git
   https://github.com/untitaker/mates.rs` to install the dev version)
2. Add `~/.cargo/bin/` to your path. The binary inside it doesn't depend on
   either Rust or Cargo, just `glibc` and `grep`.


## Usage

Set the environment variable `MATES_DIR` to your directory of `.vcf`-files.
Then run the binary with `--help` to list all commands. 

The other environment variables are:

- `MATES_EDITOR`, override the vCard editor to use. Default to `EDITOR`.
- `MATES_GREP`, override the grep binary to use. Default to `grep`. This
  command must accept a search string as first argument and a filepath as
  second one.
- `MATES_INDEX`, the filepath to the contact index. Default to `~/.mates_index`.

**Note: "mates index" must be called regularly.** Even when using mates' own
commands, the index will not be updated automatically, as this would impact UI
responsiveness massively.


## Integration

### Mutt

#### Query command in mutt (email autocompletion)

    # ~/.muttrc

    set query_command= "mates mutt-query '%s'"

    # Normally you'd have to hit Ctrl-T for completion.
    # This rebinds it to Tab.
    bind editor <Tab> complete-query
    bind editor ^T    complete


#### Create new contact from message

    # ~/.muttrc

    macro index,pager A \
        "<pipe-message>mates add | xargs mates edit<enter>" \
        "add the sender address"

With this configuration, hitting `A` when viewing a message or highlighting
it in the folder view will add it to your contacts and open the new contact in
your editor. If you clear the file, the new contact will be deleted.


### Using fuzzy finders for email selection

[selecta](https://github.com/garybernhardt/selecta) and
[fzf](https://github.com/junegunn/fzf) are tools that can be used instead of
grep to search for contacts:

    m() {
        mutt "$(MATES_GREP=selecta mates email-query)"
    }

    m() {
        mutt "$(MATES_GREP='fzf -q' mates email-query)"
    }

Selecta is much more lightweight than fzf, but fzf provides a nicer interface
on the other hand.

### Synchronization with CardDAV (Vdirsyncer)

[Vdirsyncer](https://vdirsyncer.readthedocs.org/) can be used to synchronize
mates' `MATES_DIR` with CardDAV-servers. Here is a simple example
configuration, where `MATES_DIR=~/.contacts/`:

    [pair contacts]
    a = contacts_local
    b = contacts_remote

    [storage contacts_local]
    type = filesystem
    path = ~/.contacts/
    fileext = .vcf

    [storage contacts_remote]
    type = carddav
    url = https://davserver.example.com/
    username = foouser
    password = foopass


## License

Mates is released under the MIT license, see `LICENSE` for details.
