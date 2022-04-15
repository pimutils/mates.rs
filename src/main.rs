#[macro_use]
extern crate anyhow;
use anyhow::Result;
use std::fs;
use std::io;
use std::io::{Read, Write};

use mates::app;
use mates::cli;
use mates::utils;

fn main() -> Result<()> {
    let matches = app::app().get_matches();

    let command = matches.subcommand_name().unwrap();

    let config = match cli::Configuration::new() {
        Ok(x) => x,
        Err(e) => {
            return Err(anyhow!("Error while reading configuration: {}", e));
        }
    };

    let submatches = matches
        .subcommand_matches(command)
        .expect("Internal error.");

    match command {
        "index" => {
            println!(
                "Rebuilding index file \"{}\"...",
                config.index_path.display()
            );
            cli::build_index(&config.index_path, &config.vdir_path)?;
        }
        "mutt-query" => {
            let query = submatches.value_of("query").unwrap_or("");
            cli::mutt_query(&config, &query[..])?;
        }
        "file-query" => {
            let query = submatches.value_of("query").unwrap_or("");
            cli::file_query(&config, &query[..])?;
        }
        "email-query" => {
            let query = submatches.value_of("query").unwrap_or("");
            cli::email_query(&config, &query[..])?;
        }
        "add" => {
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
        "edit" => {
            let query = submatches.value_of("file-or-query").unwrap_or("");
            cli::edit_contact(&config, &query[..])?;
        }
        _ => {
            return Err(anyhow!("Invalid command: {}", command));
        }
    };
    Ok(())
}
