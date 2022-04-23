use anyhow::Result;
use std::borrow::ToOwned;
use std::env;
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::path;

use atomicwrites::{AllowOverwrite, AtomicFile};

use crate::editor;
use crate::utils;
use crate::utils::CustomPathExt;

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

pub fn index_contact(index_path: &path::Path, contact: &utils::Contact) -> Result<()> {
    let mut index_fp = fs::OpenOptions::new()
        .append(true)
        .write(true)
        .open(&index_path)?;

    let index_entry = utils::index_item_from_contact(contact)?;
    index_fp.write_all(index_entry.as_bytes())?;
    Ok(())
}

pub fn build_index(outfile: &path::Path, dir: &path::Path) -> Result<()> {
    if !dir.is_dir() {
        return Err(anyhow!("MATES_DIR must be a directory."));
    };

    let af = AtomicFile::new(&outfile, AllowOverwrite);
    let mut errors = false;

    af.write::<(), io::Error, _>(|outf| {
        for entry in fs::read_dir(dir)? {
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
                    continue;
                }
            };

            match utils::index_item_from_contact(&contact) {
                Ok(index_string) => {
                    outf.write_all(index_string.as_bytes())?;
                }
                Err(e) => {
                    println!("Error while indexing {}: {}", pathbuf.display(), e);
                    errors = true;
                    continue;
                }
            };
        }
        Ok(())
    })?;

    if errors {
        Err(anyhow!(
            "Several errors happened while generating the index."
        ))
    } else {
        Ok(())
    }
}

pub fn edit_contact(config: &Configuration, query: &str) -> Result<()> {
    let results = if get_pwd().join(query).is_file() {
        vec![path::PathBuf::from(query)]
    } else {
        utils::file_query(config, query)?.into_iter().collect()
    };

    if results.len() < 1 {
        return Err(anyhow!("No such contact."));
    } else if results.len() > 1 {
        return Err(anyhow!("Ambiguous query."));
    }

    let fpath = &results[0];
    editor::cli_main(fpath);

    let fcontent = {
        let mut fcontent = String::new();
        let mut file = fs::File::open(fpath)?;
        file.read_to_string(&mut fcontent)?;
        fcontent
    };

    if (&fcontent[..]).trim().len() == 0 {
        fs::remove_file(fpath)?;
        return Err(anyhow!("Contact emptied, file removed."));
    };

    Ok(())
}

pub fn mutt_query(config: &Configuration, disable_first_line: bool, query: &str) -> Result<()> {
    // For some reason mutt requires an empty line
    // We need to ignore errors here, otherwise mutt's UI will glitch
    if !disable_first_line {
        println!();
    }

    if let Ok(items) = utils::index_query(config, query) {
        for item in items {
            if item.email.len() > 0 && item.name.len() > 0 {
                println!("{}\t{}", item.email, item.name);
            };
        }
    };
    Ok(())
}

pub fn file_query(config: &Configuration, query: &str) -> Result<()> {
    for path in utils::file_query(config, query)?.iter() {
        println!("{}", path.display());
    }
    Ok(())
}

pub fn email_query(config: &Configuration, query: &str) -> Result<()> {
    for item in utils::index_query(config, query)? {
        if item.name.len() > 0 && item.email.len() > 0 {
            println!("{} <{}>", item.name, item.email);
        };
    }
    Ok(())
}

pub struct Configuration {
    pub index_path: path::PathBuf,
    pub vdir_path: path::PathBuf,
    pub grep_cmd: String,
}

impl Configuration {
    pub fn new() -> Result<Configuration, String> {
        Ok(Configuration {
            index_path: match get_envvar("MATES_INDEX") {
                Some(x) => path::PathBuf::from(&x),
                None => match get_envvar("HOME") {
                    Some(home) => get_pwd().join(&home).join(".mates_index"),
                    None => return Err("Unable to determine user's home directory.".to_owned()),
                },
            },
            vdir_path: match get_envvar("MATES_DIR") {
                Some(x) => path::PathBuf::from(&x),
                None => {
                    return Err(
                        "MATES_DIR must be set to your vdir path (directory of vcf-files)."
                            .to_owned(),
                    )
                }
            },
            grep_cmd: match get_envvar("MATES_GREP") {
                Some(x) => x,
                None => "grep -i".to_owned(),
            },
        })
    }
}
