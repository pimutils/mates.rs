=====
Mates
=====

A commandline addressbook. The main goals are:

- **Few features, high extensibility**

  1. Mates operates on a directory of vCard_ files, a standardized file format
     for contacts. Because of this, another program can be used for
     synchronization with CardDAV-servers (see below).

  2. Mates doesn't come with a contact editor. It relies on other programs to
     fulfill this task (which can be configured with ``MATES_EDITOR``), by
     default it will open your text editor.

- **UI responsiveness** For completing email addresses in mutt, mates maintains
  a simple textfile with only a few fields from the vCard file, on which it
  calls ``grep``. The textfile looks like this::

      work@example.com\tExample Man\t/home/user/.contacts/exampleman.vcf
      home@example.com\tExample Man\t/home/user/.contacts/exampleman.vcf

.. _vCard: https://tools.ietf.org/html/rfc6350


Installation
============

.. image:: https://travis-ci.org/untitaker/mates.rs.svg?branch=master
    :target: https://travis-ci.org/untitaker/mates.rs

If the above button is green, mates doesn't break against Rust nightly.

On ArchLinux, simply use the mates-git_ package from the AUR.

.. _mates-git: https://aur.archlinux.org/packages/mates-git/

For a manual installation, you need to have the nightly versions of Rust_ and
Cargo_ installed.

.. _Rust: http://www.rust-lang.org/
.. _Cargo: https://crates.io/

1. Clone this repository, ``cd`` to it.
2. Run ``cargo build --release``. You could omit the release-flag, but then
   mates would run awfully slow.
3. The resulting binary is in ``./target/release/`` (``./target/`` if you built
   without the release-flag), and depends on ``glibc``, and ``grep`` in your
   ``PATH``.


Usage
=====

Set the environment variable ``MATES_DIR`` to your directory of ``.vcf``-files.
Then run the binary with ``--help`` to list all commands. 

The other environment variables are:

- ``MATES_EDITOR``, override the vCard editor to use. Default to ``EDITOR``.
- ``MATES_GREP``, override the grep binary to use. Default to ``grep``.
- ``MATES_INDEX``, the filepath to the contact index. Default to ``~/.mates_index``.

**Note: "mates index" must be called regularly.** Even when using mates' own
commands, the index will not be updated automatically, as this would impact UI
responsiveness massively.


Integration
===========

Mutt
----

Query command in mutt (email autocompletion)
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

::

      # ~/.muttrc

      set query_command= "mates mutt-query '%s'"

      # Normally you'd have to hit Ctrl-T for completion.
      # This rebinds it to Tab.
      bind editor <Tab> complete-query
      bind editor ^T    complete


Create new contact from message
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~


::

    # ~/.muttrc

    macro index,pager A \
        "<pipe-message>mates add | xargs mates edit<enter>" \
        "add the sender address"

With this configuration, hitting ``A`` when viewing a message or highlighting
it in the folder view will add it to your contacts and open the new contact in
your editor. If you clear the file, the new contact will be deleted.


Using fuzzy finders for email selection
---------------------------------------

selecta_ and fzf_ are tools that can be used instead of grep to search for
contacts::

    m() {
        mutt "$(MATES_GREP=selecta mates email-query)"
    }

    m() {
        mutt "$(MATES_GREP='fzf -q' mates email-query)"
    }

Selecta is much more lightweight than fzf, but fzf provides a nicer interface
on the other hand.

.. _selecta: https://github.com/garybernhardt/selecta
.. _fzf: https://github.com/junegunn/fzf

.. _vdirsyncer-integration:

Synchronization with CardDAV (Vdirsyncer)
-----------------------------------------

Vdirsyncer_ can be used to synchronize mates' ``MATES_DIR`` with
CardDAV-servers. Here is a simple example configuration, where
``MATES_DIR=~/.contacts/``::

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


.. _Vdirsyncer: https://vdirsyncer.readthedocs.org/

License
=======

Mates is released under the MIT license, see ``LICENSE`` for details.
