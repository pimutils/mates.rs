#[macro_use]
extern crate anyhow;
use anyhow::Result;
use clap::{Arg, Command};
use std::io;
use std::io::Read;

use mates::cli;
use mates::utils;

fn main() -> Result<()> {
    let app = Command::new("mates")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Markus Unterwaditzer")
        .about("A simple commandline addressbook")
        .subcommand_required(true)
        .subcommand(Command::new("index").about("Rewrite/create the index"))
        .subcommand(
            Command::new("mutt-query")
                .about("Search for contact, output is usable for mutt's query_command.")
                .arg(
                    Arg::new("disable-empty-line")
                        .long("disable-empty-line")
                        .help("Disable printing an empty first line"),
                )
                .arg(Arg::new("query").required(true)),
        )
        .subcommand(
            Command::new("file-query")
                .about("Search for contact, return just the filename.")
                .arg(Arg::new("query").required(true)),
        )
        .subcommand(
            Command::new("email-query")
                .about("Search for contact, return \"name <email>\".")
                .arg(Arg::new("query").required(true)),
        )
        .subcommand(
            Command::new("add")
                .about("Manually add a contacts email id and full name.")
                .arg(Arg::new("email").required(true))
                .arg(Arg::new("fullname")),
        )
        .subcommand(
            Command::new("add-email")
                .about("Take mail from stdin, add sender to contacts. Print filename."),
        )
        .subcommand(
            Command::new("edit")
                .about("Open contact (given by filepath or search-string) interactively.")
                .arg(Arg::new("file-or-query").required(true)),
        )
        .get_matches();

    let config = match cli::Configuration::new() {
        Ok(x) => x,
        Err(e) => {
            return Err(anyhow!("Error while reading configuration: {}", e));
        }
    };

    match app.subcommand() {
        Some(("index", _)) => {
            println!(
                "Rebuilding index file \"{}\"...",
                config.index_path.display()
            );
            cli::build_index(&config.index_path, &config.vdir_path)?;
        }
        Some(("mutt-query", args)) => {
            if let Some(value) = args.value_of("query") {
                cli::mutt_query(&config, args.is_present("disable-empty-line"), value)?
            }
        }
        Some(("file-query", args)) => {
            if let Some(value) = args.value_of("query") {
                cli::file_query(&config, value)?
            }
        }
        Some(("email-query", args)) => {
            if let Some(value) = args.value_of("query") {
                cli::email_query(&config, value)?
            }
        }
        Some(("add", args)) => {
            let contact = utils::Contact::generate(
                args.value_of("fullname"),
                args.value_of("email"),
                &config.vdir_path,
            );
            contact.write_create()?;

            cli::index_contact(&config.index_path, &contact)?
        }
        Some(("add-email", _)) => {
            let stdin = io::stdin();
            let mut email = String::new();
            stdin.lock().read_to_string(&mut email)?;
            let contact = utils::add_contact_from_email(&config.vdir_path, &email[..])?;
            println!("{}", contact.path.display());

            cli::index_contact(&config.index_path, &contact)?
        }
        Some(("edit", args)) => {
            if let Some(value) = args.value_of("file-or-query") {
                cli::edit_contact(&config, value)?
            }
        }
        _ => (),
    }

    Ok(())
}
