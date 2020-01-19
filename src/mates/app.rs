use clap::{App, AppSettings, Arg, SubCommand};

pub fn app() -> App<'static, 'static> {
    App::new("mates")
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
}
