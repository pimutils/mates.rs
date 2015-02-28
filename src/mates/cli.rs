use std::os;
use std::env;
use std::old_io;
use std::old_io::fs::PathExtensions;
use std::borrow::ToOwned;

use atomicwrites::{GenericAtomicFile,AtomicFile,AllowOverwrite};

use utils;

macro_rules! main_try {
    ($result: expr, $errmsg: expr) => (
        match $result {
            Ok(m) => m,
            Err(e) => {
                if e.desc.len() > 0 {
                    writeln!(&mut old_io::stdio::stderr(), "{}: {}", $errmsg, e).unwrap();
                };
                env::set_exit_status(1);
                return;
            }
        }
    )
}

fn get_pwd() -> Path {
    match os::getcwd() {
        Ok(x) => x,
        Err(e) => panic!(format!("Failed to get current working directory: {}", e))
    }
}

fn get_envvar(key: &str) -> Option<String> {
    match env::var(key) {
        Ok(x) => Some(x),
        Err(e) => match e {
            env::VarError::NotPresent => None,
            env::VarError::NotUnicode(_) => panic!(format!("{} is not unicode.", key))
        }
    }
}

fn build_index(outfile: &Path, dir: &Path) -> old_io::IoResult<()> {
    if !dir.is_dir() {
        return Err(old_io::IoError {
            kind: old_io::MismatchedFileTypeForOperation,
            desc: "MATES_DIR must be a directory.",
            detail: None
        });
    };

    let af: AtomicFile = GenericAtomicFile::new(outfile, AllowOverwrite);
    let entries = try!(old_io::fs::readdir(dir));
    let mut errors = false;

    try!(af.write(|outf| {
        for entry in entries.iter() {
            if !entry.is_file() || !entry.filename_str().unwrap_or("").ends_with(".vcf") {
                continue;
            }

            let contact = match utils::Contact::from_file(entry.clone()) {
                Ok(x) => x,
                Err(e) => {
                    println!("Error while reading {}: {}", entry.display(), e);
                    errors = true;
                    continue
                }
            };

            match utils::index_item_from_contact(&contact) {
                Ok(index_string) => {
                    try!(outf.write_str(index_string.as_slice()));
                },
                Err(e) => {
                    println!("Error while indexing {}: {}", entry.display(), e);
                    errors = true;
                    continue
                }
            };
        };
        Ok(())
    }));

    if errors {
        Err(old_io::IoError {
            kind: old_io::OtherIoError,
            desc: "Several errors happened while generating the index.",
            detail: None
        })
    } else {
        Ok(())
    }
}

pub fn cli_main() {
    let mut args = env::args();
    let program = args.next().unwrap_or("mates".to_string());

    let help = format!("Usage: {} COMMAND
Commands:
    index:
        Rewrite/create the index.
    mutt-query <query>:
        Search for contact, output is usable for mutt's query_command.
    file-query <query>:
        Search for contact, return just the filename.
    email-query <query>:
        Search for contact, return \"name <email>\".
    add:
        Take mail from stdin, add sender to contacts. Print filename.
    edit <file-or-query>:
        Open contact (given by filepath or search-string) in $MATES_EDITOR. If the file is cleared,
        the contact is removed. As a further convenience it also clears stdin, which is necessary
        for editors and most interactive programs to not act weird when piped to.",
        program);

    let print_help = |&:| {
        println!("{}", help);
    };

    let command = match args.next() {
        Some(x) => x,
        None => {
            print_help();
            env::set_exit_status(1);
            return;
        }
    };

    if command == "--help" || command == "help" || command == "-h" {
        print_help();
        return;
    }

    let config = match Configuration::new() {
        Ok(x) => x,
        Err(e) => {
            println!("Error while reading configuration: {}", e);
            env::set_exit_status(1);
            return;
        }
    };

    match command.as_slice() {
        "index" => {
            println!("Rebuilding index file \"{}\"...", config.index_path.display());
            main_try!(build_index(&config.index_path, &config.vdir_path), "Failed to build index");
        },
        "mutt-query" => {
            let query = args.next().unwrap_or("".to_string());
            main_try!(mutt_query(&config, query.as_slice()), "Failed to execute grep");
        },
        "file-query" => {
            let query = args.next().unwrap_or("".to_string());
            main_try!(file_query(&config, query.as_slice()), "Failed to execute grep");
        },
        "email-query" => {
            let query = args.next().unwrap_or("".to_string());
            main_try!(email_query(&config, query.as_slice()), "Failed to execute grep");
        },
        "add" => {
            let mut stdin = old_io::stdin();
            let email = main_try!(stdin.lock().read_to_string(), "Failed to read email");
            let contact = main_try!(utils::add_contact_from_email(
                &config.vdir_path,
                email.as_slice()
            ), "Failed to add contact");
            println!("{}", contact.path.display());

            let mut index_fp = main_try!(old_io::File::open_mode(
                &config.index_path,
                old_io::Append,
                old_io::Write),
                "Failed to open index"
            );

            let index_entry = main_try!(utils::index_item_from_contact(&contact), "Failed to generate index");
            main_try!(index_fp.write_str(index_entry.as_slice()), "Failed to write to index");
        },
        "edit" => {
            let query = args.next().unwrap_or("".to_string());
            main_try!(edit_contact(&config, query.as_slice()), "Failed to edit contact");
        },
        _ => {
            println!("Invalid command: {}", command);
            print_help();
            env::set_exit_status(1);
        }
    };
}

fn edit_contact(config: &Configuration, query: &str) -> old_io::IoResult<()> {
    let results = if get_pwd().join(query).is_file() {
        vec![Path::new(query)]
    } else {
        try!(utils::file_query(config, query)).into_iter().collect()
    };

    if results.len() < 1 {
        return Err(old_io::IoError {
            kind: old_io::OtherIoError,
            desc: "No such contact.",
            detail: None
        })
    } else if results.len() > 1 {
        return Err(old_io::IoError {
            kind: old_io::OtherIoError,
            desc: "Ambiguous query.",
            detail: None
        })
    }

    let fpath = &results[0];
    let mut process = try!(old_io::Command::new("sh")
        .arg("-c")
        // clear stdin, http://unix.stackexchange.com/a/77593
        .arg(format!("$0 -- \"$1\" < $2"))
        .arg(config.editor_cmd.as_slice())
        .arg(fpath.as_str().unwrap())
        .arg("/dev/tty")
        .stdin(old_io::process::InheritFd(0))
        .stdout(old_io::process::InheritFd(1))
        .stderr(old_io::process::InheritFd(2))
        .spawn());

    try!(utils::handle_process(&mut process));

    if try!(old_io::File::open(fpath).read_to_string()).as_slice().trim().len() == 0 {
        try!(old_io::fs::unlink(fpath));
        return Err(old_io::IoError {
            kind: old_io::OtherIoError,
            desc: "Contact emptied, file removed.",
            detail: None
        });
    };

    Ok(())
}

fn mutt_query<'a>(config: &Configuration, query: &str) -> old_io::IoResult<()> {
    println!("");  // For some reason mutt requires an empty line
    for item in try!(utils::index_query(config, query)) {
        if item.email.len() > 0 && item.name.len() > 0 {
            println!("{}\t{}", item.email, item.name);
        };
    };
    Ok(())
}

fn file_query<'a>(config: &Configuration, query: &str) -> old_io::IoResult<()> {
    for path in try!(utils::file_query(config, query)).iter() {
        println!("{}", path.display());
    };
    Ok(())
}

fn email_query<'a>(config: &Configuration, query: &str) -> old_io::IoResult<()> {
    for item in try!(utils::index_query(config, query)) {
        if item.name.len() > 0 && item.email.len() > 0 {
            println!("{} <{}>", item.name, item.email);
        };
    };
    Ok(())
}

pub struct Configuration {
    pub index_path: Path,
    pub vdir_path: Path,
    pub editor_cmd: String,
    pub grep_cmd: String
}

impl Configuration {
    pub fn new() -> Result<Configuration, String> {
        Ok(Configuration {
            index_path: match get_envvar("MATES_INDEX") {
                Some(x) => Path::new(x),
                None => match get_envvar("HOME") {
                    Some(home) => get_pwd().join(home).join(".mates_index"),
                    None => return Err("Unable to determine user's home directory.".to_owned())
                }
            },
            vdir_path: match get_envvar("MATES_DIR") {
                Some(x) => Path::new(x),
                None => return Err("MATES_DIR must be set to your vdir path (directory of vcf-files).".to_owned())
            },
            editor_cmd: match get_envvar("MATES_EDITOR") {
                Some(x) => x,
                None => match get_envvar("EDITOR") {
                    Some(x) => x,
                    None => return Err("MATES_EDITOR or EDITOR must be set.".to_owned())
                }
            },
            grep_cmd: match get_envvar("MATES_GREP") {
                Some(x) => x,
                None => "grep -i".to_owned()
            }
        })
    }
}
