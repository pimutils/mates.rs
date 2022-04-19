#[macro_use]
extern crate anyhow;
use anyhow::Result;
use std::fs;
use std::io;
use std::io::{Read, Write};
use clap::{App, AppSettings, Arg, SubCommand};

use mates::cli;
use mates::utils;

fn main() -> Result<()> {
    let app = App::new("mates")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Markus Unterwaditzer")
        .about("A simple commandline addressbook")
        .setting(AppSettings::SubcommandRequired)
        .subcommand(SubCommand::with_name("index").about("Rewrite/create the index"))
        .subcommand(
            SubCommand::with_name("mutt-query")
                .about("Search for contact, output is usable for mutt's query_command.")
                .arg(Arg::with_name("query").index(1)),
        )
        .subcommand(
            SubCommand::with_name("file-query")
                .about("Search for contact, return just the filename.")
                .arg(Arg::with_name("query").index(1)),
        )
        .subcommand(
            SubCommand::with_name("email-query")
                .about("Search for contact, return \"name <email>\".")
                .arg(Arg::with_name("query").index(1)),
        )
        .subcommand(
            SubCommand::with_name("add")
                .about("Take mail from stdin, add sender to contacts. Print filename."),
        )
        .subcommand(
            SubCommand::with_name("edit")
                .about("Open contact (given by filepath or search-string) interactively.")
                .arg(Arg::with_name("file-or-query").index(1)),
        )
        .get_matches();

    let config = match cli::Configuration::new() {
        Ok(x) => x,
        Err(e) => {
            return Err(anyhow!("Error while reading configuration: {}", e));
        }
    };

    match app.subcommand() {
        ("index", Some(_subs)) => {
            println!(
                "Rebuilding index file \"{}\"...",
                config.index_path.display()
            );
            cli::build_index(&config.index_path, &config.vdir_path)?;
        }
        ("mutt-query", Some(subs)) => {
            if let Some(value) = subs.value_of("query") {
                cli::mutt_query(&config, value)?
            }
        }
        ("file-query", Some(subs)) => {
            if let Some(value) = subs.value_of("query") {
                cli::file_query(&config, value)?
            }
        }
        ("email-query", Some(subs)) => {
            if let Some(value) = subs.value_of("query") {
                cli::email_query(&config, value)?
            }
        }
        ("add", Some(..)) => {
            let stdin = io::stdin();
            let mut email = String::new();
            stdin.lock().read_to_string(&mut email)?;
            let contact = utils::add_contact_from_email(&config.vdir_path, &email[..])?;
            println!("{}", contact.path.display());

            let mut index_fp = fs::OpenOptions::new()
                .append(true)
                .write(true)
                .open(&config.index_path)?;

            let index_entry = utils::index_item_from_contact(&contact)?;
            index_fp.write_all(index_entry.as_bytes())?;
        }
        ("edit", Some(subs)) => {
            if let Some(value) = subs.value_of("file-or-query") {
                cli::edit_contact(&config, value)?
            }
        }
        _ => (),
    }

    Ok(())
}
