use std::fs;
use std::io;
use std::io::{Read,Write};
use std::process;
use std::path;
use std::env;
use std::borrow::ToOwned;
use std::error::Error;

use clap::{Arg,App,SubCommand};
use atomicwrites::{AtomicFile,AllowOverwrite};

use utils;
use utils::CustomPathExt;

macro_rules! main_try {
    ($result: expr, $errmsg: expr) => (
        match $result {
            Ok(m) => m,
            Err(e) => {
                if e.description().len() > 0 {
                    writeln!(&mut io::stderr(), "{}: {}", $errmsg, e).unwrap();
                };
                env::set_exit_status(1);
                return;
            }
        }
    )
}

fn get_pwd() -> path::PathBuf {
    env::current_dir().ok().expect("Failed to get CWD")
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

fn build_index(outfile: &path::Path, dir: &path::Path) -> io::Result<()> {
    if !dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MATES_DIR must be a directory.",
        ));
    };

    let af = AtomicFile::new(&outfile, AllowOverwrite);
    let mut errors = false;

    try!(af.write(|outf| {
        for entry in try!(fs::read_dir(dir)) {
            let entry = match entry {
                Ok(x) => x,
                Err(e) => {
                    println!("Error while listing directory: {}", e);
                    errors = true;
                    continue;
                }
            };

            let pathbuf = entry.path();

            if pathbuf.str_extension().unwrap_or("") != "vcf" || !pathbuf.is_file() {
                continue;
            };

            let contact = match utils::Contact::from_file(&pathbuf) {
                Ok(x) => x,
                Err(e) => {
                    println!("Error while reading {}: {}", pathbuf.display(), e);
                    errors = true;
                    continue
                }
            };

            match utils::index_item_from_contact(&contact) {
                Ok(index_string) => {
                    try!(outf.write_all(index_string.as_bytes()));
                },
                Err(e) => {
                    println!("Error while indexing {}: {}", pathbuf.display(), e);
                    errors = true;
                    continue
                }
            };
        };
        Ok(())
    }));

    if errors {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "Several errors happened while generating the index.",
        ))
    } else {
        Ok(())
    }
}

pub fn cli_main() {
    let matches = App::new("mates")
        .version("0.0.1")  // FIXME: Use package metadata
        .author("Markus Unterwaditzer")
        .about("A simple commandline addressbook")
        .subcommand(SubCommand::new("index")
                    .about("Rewrite/create the index"))
        .subcommand(SubCommand::new("mutt-query")
                    .about("Search for contact, output is usable for mutt's query_command.")
                    .arg(Arg::new("query").index(1)))
        .subcommand(SubCommand::new("file-query")
                    .about("Search for contact, return just the filename.")
                    .arg(Arg::new("query").index(1)))
        .subcommand(SubCommand::new("email-query")
                    .about("Search for contact, return \"name <email>\".")
                    .arg(Arg::new("query").index(1)))
        .subcommand(SubCommand::new("add")
                    .about("Take mail from stdin, add sender to contacts. Print filename."))
        .subcommand(SubCommand::new("edit")
                    .about(
                        "Open contact (given by filepath or search-string) in $MATES_EDITOR. If
                        the file is cleared, the contact is removed. As a further convenience it 
                        also clears stdin, which is necessary for editors and most interactive 
                        programs to not act weird when piped to."
                    )
                    .arg(Arg::new("file-or-query").index(1)))
        .get_matches();

    let command = match matches.subcommand_name() {
        Some(x) => x,
        None => {
            println!("Command required. See --help for usage.");
            env::set_exit_status(1);
            return;
        }
    };

    let config = match Configuration::new() {
        Ok(x) => x,
        Err(e) => {
            println!("Error while reading configuration: {}", e);
            env::set_exit_status(1);
            return;
        }
    };

    let submatches = matches.subcommand_matches(command).expect("Internal error.");

    match command {
        "index" => {
            println!("Rebuilding index file \"{}\"...", config.index_path.display());
            main_try!(build_index(&config.index_path, &config.vdir_path), "Failed to build index");
        },
        "mutt-query" => {
            let query = submatches.value_of("query").unwrap_or("");
            main_try!(mutt_query(&config, &query[..]), "Failed to execute grep");
        },
        "file-query" => {
            let query = submatches.value_of("query").unwrap_or("");
            main_try!(file_query(&config, &query[..]), "Failed to execute grep");
        },
        "email-query" => {
            let query = submatches.value_of("query").unwrap_or("");
            main_try!(email_query(&config, &query[..]), "Failed to execute grep");
        },
        "add" => {
            let stdin = io::stdin();
            let mut email = String::new();
            main_try!(stdin.lock().read_to_string(&mut email), "Failed to read email");
            let contact = main_try!(utils::add_contact_from_email(
                &config.vdir_path,
                &email[..]
            ), "Failed to add contact");
            println!("{}", contact.path.display());

            let mut index_fp = main_try!(
                fs::OpenOptions::new()
                .append(true)
                .write(true)
                .open(&config.index_path),
                "Failed to open index"
            );

            let index_entry = main_try!(utils::index_item_from_contact(&contact), "Failed to generate index");
            main_try!(index_fp.write_all(index_entry.as_bytes()), "Failed to write to index");
        },
        "edit" => {
            let query = submatches.value_of("file-or-query").unwrap_or("");
            main_try!(edit_contact(&config, &query[..]), "Failed to edit contact");
        },
        _ => {
            println!("Invalid command: {}", command);
            env::set_exit_status(1);
        }
    };
}

fn edit_contact(config: &Configuration, query: &str) -> io::Result<()> {
    let results = if get_pwd().join(query).is_file() {
        vec![path::PathBuf::from(query)]
    } else {
        try!(utils::file_query(config, query)).into_iter().collect()
    };

    if results.len() < 1 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "No such contact.",
        ))
    } else if results.len() > 1 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Ambiguous query.",
        ))
    }

    let fpath = &results[0];
    let mut process = try!(process::Command::new("sh")
        .arg("-c")
        // clear stdin, http://unix.stackexchange.com/a/77593
        .arg("$0 \"$1\" < $2")
        .arg(&config.editor_cmd[..])
        .arg(fpath.as_os_str())
        .arg("/dev/tty")
        .stdin(process::Stdio::inherit())
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .spawn());

    try!(utils::handle_process(&mut process));

    let fcontent = {
        let mut fcontent = String::new();
        let mut file = try!(fs::File::open(fpath));
        try!(file.read_to_string(&mut fcontent));
        fcontent
    };

    if (&fcontent[..]).trim().len() == 0 {
        try!(fs::remove_file(fpath));
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Contact emptied, file removed.",
        ));
    };

    Ok(())
}

fn mutt_query<'a>(config: &Configuration, query: &str) -> io::Result<()> {
    println!("");  // For some reason mutt requires an empty line
    for item in try!(utils::index_query(config, query)) {
        if item.email.len() > 0 && item.name.len() > 0 {
            println!("{}\t{}", item.email, item.name);
        };
    };
    Ok(())
}

fn file_query<'a>(config: &Configuration, query: &str) -> io::Result<()> {
    for path in try!(utils::file_query(config, query)).iter() {
        println!("{}", path.display());
    };
    Ok(())
}

fn email_query<'a>(config: &Configuration, query: &str) -> io::Result<()> {
    for item in try!(utils::index_query(config, query)) {
        if item.name.len() > 0 && item.email.len() > 0 {
            println!("{} <{}>", item.name, item.email);
        };
    };
    Ok(())
}

pub struct Configuration {
    pub index_path: path::PathBuf,
    pub vdir_path: path::PathBuf,
    pub editor_cmd: String,
    pub grep_cmd: String
}

impl Configuration {
    pub fn new() -> Result<Configuration, String> {
        Ok(Configuration {
            index_path: match get_envvar("MATES_INDEX") {
                Some(x) => path::PathBuf::from(&x),
                None => match get_envvar("HOME") {
                    Some(home) => get_pwd().join(&home).join(".mates_index"),
                    None => return Err("Unable to determine user's home directory.".to_owned())
                }
            },
            vdir_path: match get_envvar("MATES_DIR") {
                Some(x) => path::PathBuf::from(&x),
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
