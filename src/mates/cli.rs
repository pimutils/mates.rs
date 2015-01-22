use std::os;
use std::io;
use std::collections::HashMap;
use std::io::fs::PathExtensions;
use std::borrow::ToOwned;

use atomicwrites::{AtomicFile,AllowOverwrite};

use utils::{
    Contact, index_query, index_item_from_contact, parse_from_header, read_sender_from_email
};

macro_rules! main_try {
    ($result: expr, $errmsg: expr) => (
        match $result {
            Ok(m) => m,
            Err(e) => {
                println!("{}: {}", $errmsg, e);
                os::set_exit_status(1);
                return;
            }
        }
    )
}

fn build_index(outfile: &Path, dir: &Path) -> io::IoResult<()> {
    if !dir.is_dir() {
        return Err(io::IoError {
            kind: io::MismatchedFileTypeForOperation,
            desc: "MATES_DIR must be a directory.",
            detail: None
        });
    };

    let af = AtomicFile::new(outfile, AllowOverwrite, None);
    let entries = try!(io::fs::readdir(dir));
    let mut errors = false;

    try!(af.write(|&mut: outf| {
        for entry in entries.iter() {
            if !entry.is_file() || !entry.filename_str().unwrap_or("").ends_with(".vcf") {
                continue;
            }

            let contact = match Contact::from_file(entry.clone()) {
                Ok(x) => x,
                Err(e) => {
                    println!("Error while reading {}: {}", entry.display(), e);
                    errors = true;
                    continue
                }
            };

            match index_item_from_contact(&contact) {
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
        Err(io::IoError {
            kind: io::OtherIoError,
            desc: "Several errors happened while generating the index.",
            detail: None
        })
    } else {
        Ok(())
    }
}

pub fn cli_main() {
    let mut args = os::args().into_iter();
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
        Open contact (given by filepath or search-string) in $MATES_EDITOR. If
        the file is cleared, the contact is removed.", program);

    let print_help = |&:| {
        println!("{}", help);
    };

    let command = match args.next() {
        Some(x) => x,
        None => {
            print_help();
            os::set_exit_status(1);
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
            os::set_exit_status(1);
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
            let contact = main_try!(add_contact(&config.vdir_path), "Failed to add contact");
            println!("{}", contact.path.display());

            let mut index_fp = main_try!(io::File::open_mode(
                &config.index_path,
                io::Append,
                io::Write),
                "Failed to open index"
            );

            let index_entry = main_try!(index_item_from_contact(&contact), "Failed to generate index");
            main_try!(index_fp.write_str(index_entry.as_slice()), "Failed to write to index");
        },
        "edit" => {
            let query = args.next().unwrap_or("".to_string());
            main_try!(edit_contact(&config, query.as_slice()), "Failed to edit contact");
        },
        _ => {
            println!("Invalid command: {}", command);
            print_help();
            os::set_exit_status(1);
        }
    };
}

fn add_contact(contact_dir: &Path) -> io::IoResult<Contact> {
    let stdin = try!(io::stdin().lock().read_to_string());
    let from_header = match read_sender_from_email(stdin.as_slice()) {
        Some(x) => x,
        None => return Err(io::IoError {
            kind: io::InvalidInput,
            desc: "Couldn't find From-header in email.",
            detail: None
        })
    };
    let (fullname, email) = parse_from_header(&from_header);
    let contact = Contact::generate(fullname, email, contact_dir);
    try!(contact.write_create());
    Ok(contact)
}

fn edit_contact(config: &Configuration, query: &str) -> Result<(), String> {

    let results = {
        if config.vdir_path.join(query).is_file() {
            vec![query.to_string()]
        } else {
            let results_iter = match index_query(config, query) {
                Ok(x) => x,
                Err(e) => return Err(format!("Error while fetching index: {}", e))
            };

            results_iter.filter_map(|x| {
                if x.filepath.len() > 0 {
                    Some(x.filepath)
                } else {
                    None
                }}).collect()
        }
    };

    if results.len() < 1 {
        return Err("No such contact.".to_string());
    } else if results.len() > 1 {
        return Err("Ambiguous query.".to_string());
    }

    let fpath = results[0].as_slice();
    let mut process = match io::Command::new("sh")
        .arg("-c")
        // clear stdin, http://unix.stackexchange.com/a/77593
        .arg(format!("$0 -- \"$1\" < $2"))
        .arg(config.editor_cmd.as_slice())
        .arg(fpath)
        .arg("/dev/tty")
        .stdin(io::process::InheritFd(0))
        .stdout(io::process::InheritFd(1))
        .stderr(io::process::InheritFd(2))
        .spawn() {
            Ok(x) => x,
            Err(e) => return Err(format!("Error while invoking editor: {}", e))
        };

    match process.wait() {
        Ok(_) => (),
        Err(e) => return Err(format!("Error while invoking editor: {}", e))
    };

    if match io::File::open(&Path::new(fpath)).read_to_string() {
        Ok(x) => x,
        Err(e) => return Err(format!("File can't be read after user edited it: {}", e))
    }.as_slice().trim().len() == 0 {
        return Err(format!("Contact emptied, file removed."));
    };

    Ok(())
}

fn mutt_query<'a>(config: &Configuration, query: &str) -> io::IoResult<()> {
    println!("");  // For some reason mutt requires an empty line
    for item in try!(index_query(config, query)) {
        if item.email.len() > 0 && item.name.len() > 0 {
            println!("{}\t{}\t{}", item.email, item.name, item.filepath);
        };
    };
    Ok(())
}

fn file_query<'a>(config: &Configuration, query: &str) -> io::IoResult<()> {
    for item in try!(index_query(config, query)) {
        if item.filepath.len() > 0 {
            println!("{}", item.filepath)
        };
    };
    Ok(())
}

fn email_query<'a>(config: &Configuration, query: &str) -> io::IoResult<()> {
    for item in try!(index_query(config, query)) {
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
    pub fn from_env(env: Vec<(String, String)>) -> Result<Configuration, String> {
        let mut dict = HashMap::new();
        dict.extend(env.into_iter().filter(|&(_, ref v)| v.len() > 0));
        Ok(Configuration {
            index_path: match dict.remove("MATES_INDEX") {
                Some(x) => Path::new(x),
                None => match dict.get("HOME") {
                    Some(home) => {
                        os::make_absolute(&Path::new(home).join(".mates_index")).unwrap()
                    },
                    None => return Err("Unable to determine user's home directory.".to_owned())
                }
            },
            vdir_path: match dict.remove("MATES_DIR") {
                Some(x) => Path::new(x),
                None => return Err("MATES_DIR must be set to your vdir path (directory of vcf-files).".to_owned())
            },
            editor_cmd: match dict.remove("MATES_EDITOR") {
                Some(x) => x,
                None => match dict.remove("EDITOR") {
                    Some(x) => x,
                    None => return Err("MATES_EDITOR or EDITOR must be set.".to_owned())
                }
            },
            grep_cmd: match dict.remove("MATES_GREP") {
                Some(x) => x,
                None => "grep".to_owned()
            }
        })
    }

    pub fn new() -> Result<Configuration, String> {
        Configuration::from_env(os::env())
    }
}

