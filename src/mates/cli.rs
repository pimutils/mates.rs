use std::borrow::ToOwned;
use std::env;
use std::error::Error;
use std::fmt;use std::fs;
use std::io::{Read,Write};
use std::io;
use std::path;
use std::process;

use atomicwrites::{AtomicFile,AllowOverwrite};

use utils;
use utils::CustomPathExt;
use app;
use editor;


#[inline]
fn get_pwd() -> path::PathBuf {
    env::current_dir().ok().expect("Failed to get CWD")
}

#[inline]
fn get_envvar(key: &str) -> Option<String> {
    match env::var(key) {
        Ok(x) => Some(x),
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => panic!("{} is not unicode.", key),
    }
}

fn build_index(outfile: &path::Path, dir: &path::Path) -> MainResult<()> {
    if !dir.is_dir() {
        return Err(MainError::new("MATES_DIR must be a directory.").into());
    };

    let af = AtomicFile::new(&outfile, AllowOverwrite);
    let mut errors = false;

    try!(af.write::<(), io::Error, _>(|outf| {
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
        Err(MainError::new("Several errors happened while generating the index.").into())
    } else {
        Ok(())
    }
}

pub fn cli_main() {
    match cli_main_raw() {
        Err(e) => {
            writeln!(&mut io::stderr(), "{}", e).unwrap();
            process::exit(1);
        },
        _ => ()
    };
}

pub fn cli_main_raw() -> MainResult<()> {
    let matches = app::app().get_matches();

    let command = matches.subcommand_name().unwrap();

    let config = match Configuration::new() {
        Ok(x) => x,
        Err(e) => {
            return Err(MainError::new(format!("Error while reading configuration: {}", e)).into());
        }
    };

    let submatches = matches.subcommand_matches(command).expect("Internal error.");

    match command {
        "index" => {
            println!("Rebuilding index file \"{}\"...", config.index_path.display());
            try!(build_index(&config.index_path, &config.vdir_path));
        },
        "mutt-query" => {
            let query = submatches.value_of("query").unwrap_or("");
            try!(mutt_query(&config, query));
        },
        "file-query" => {
            let query = submatches.value_of("query").unwrap_or("");
            try!(file_query(&config, query));
        },
        "email-query" => {
            let query = submatches.value_of("query").unwrap_or("");
            try!(email_query(&config, query));
        },
        "add" => {
            let stdin = io::stdin();
            let mut email = String::new();
            try!(stdin.lock().read_to_string(&mut email));
            let contact = try!(utils::add_contact_from_email(
                &config.vdir_path,
                &email[..]
            ));
            println!("{}", contact.path.display());

            let mut index_fp = try!(fs::OpenOptions::new()
                                    .append(true)
                                    .write(true)
                                    .open(&config.index_path));

            let index_entry = try!(utils::index_item_from_contact(&contact));
            try!(index_fp.write_all(index_entry.as_bytes()));
        },
        "edit" => {
            let query = submatches.value_of("file-or-query").unwrap_or("");
            try!(edit_contact(&config, query));
        },
        _ => {
            return Err(MainError::new(format!("Invalid command: {}", command)).into());
        }
    };
    Ok(())
}

fn edit_contact(config: &Configuration, query: &str) -> MainResult<()> {
    let results = if get_pwd().join(query).is_file() {
        vec![path::PathBuf::from(query)]
    } else {
        try!(utils::file_query(config, query)).into_iter().collect()
    };

    if results.is_empty() {
        return Err(MainError::new("No such contact.").into());
    } else if results.len() > 1 {
        return Err(MainError::new("Ambiguous query.").into());
    }

    let fpath = &results[0];
    editor::cli_main(fpath);

    let fcontent = {
        let mut fcontent = String::new();
        let mut file = try!(fs::File::open(fpath));
        try!(file.read_to_string(&mut fcontent));
        fcontent
    };

    if (&fcontent[..]).trim().is_empty() {
        try!(fs::remove_file(fpath));
        return Err(MainError::new("Contact emptied, file removed.").into());
    };

    Ok(())
}

fn mutt_query<'a>(config: &Configuration, query: &str) -> MainResult<()> {
    println!();  // For some reason mutt requires an empty line
    // We need to ignore errors here, otherwise mutt's UI will glitch
    if let Ok(items) = utils::index_query(config, query) {
        for item in items {
            if !item.email.is_empty() && !item.name.is_empty() {
                println!("{}\t{}", item.email, item.name);
            };
        };
    };
    Ok(())
}

fn file_query<'a>(config: &Configuration, query: &str) -> MainResult<()> {
    for path in try!(utils::file_query(config, query)).iter() {
        println!("{}", path.display());
    };
    Ok(())
}

fn email_query<'a>(config: &Configuration, query: &str) -> MainResult<()> {
    for item in try!(utils::index_query(config, query)) {
        if !item.name.is_empty() && !item.email.is_empty() {
            println!("{} <{}>", item.name, item.email);
        };
    };
    Ok(())
}

pub struct Configuration {
    pub index_path: path::PathBuf,
    pub vdir_path: path::PathBuf,
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
            grep_cmd: match get_envvar("MATES_GREP") {
                Some(x) => x,
                None => "grep -i".to_owned()
            }
        })
    }
}


#[derive(PartialEq, Eq, Debug)]
pub struct MainError {
    desc: String,
}

pub type MainResult<T> = Result<T, Box<dyn Error>>;

impl Error for MainError {
    fn description(&self) -> &str {
        &self.desc[..]
    }

    fn cause(&self) -> Option<&dyn Error> {
        None
    }
}

impl fmt::Display for MainError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.description().fmt(f)
    }
}

impl MainError {
    pub fn new<T: Into<String>>(desc: T) -> Self {
        MainError {
            desc: desc.into(),
        }
    }
}
