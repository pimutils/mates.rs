use getopts::{optflag,optopt,getopts,usage};
use std::os;
use std::collections::HashMap;
use std::io;
use std::io::fs::PathExtensions;

use item::parse_item;

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

fn get_env() -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.extend(os::env().into_iter().filter(|&(ref key, ref value)| {
        key.as_slice().starts_with("MATES_") && value.len() > 0
    }));
    env
}


fn from_env<'a>(env: &'a HashMap<String, String>, key: &str) -> Option<&'a String> {
    env.find_equiv(&key)
}


fn expect_env<'a>(env: &'a HashMap<String, String>, key: &str) -> &'a String {
    from_env(env, key).expect(
        format!("The {} environment variable must be set.", key).as_slice()
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

    let mut outf = io::File::create(outfile);
    let entries = try!(io::fs::readdir(dir));
    for entry in entries.iter() {
        if !entry.is_file() {
            continue;
        }
        let itemstr = try!(io::File::open(entry).read_to_string());
        let item = parse_item(&itemstr);
        let name = item.single_value(&"FN".into_string());
        match item.all_values(&"EMAIL".into_string()) {
            Some(emails) => {
                for email in emails.iter() {
                    try!(outf.write_str(
                        format!("{}\t{}\n", email.get_raw_value(), name).as_slice()
                    ))
                };
            },
            None => ()
        };
    };
    return Ok(());
}


pub fn cli_main() {
    let args = os::args();

    let program = args[0].as_slice();
    let opts = [
        optflag("i", "index", "Create index."),
        optflag("h", "help", "Print help."),
        optopt("m", "mutt-search", "Search in index, for mutt search.", "")
    ];

    let matches = main_try!(getopts(args.tail(), opts), "Failed to parse arguments");

    let env = get_env();

    let print_usage = || {
        println!("{}", usage(program, opts));
        println!("Environment variables:");
        println!("- MATES_INDEX: Path to index file, which is basically a cache of all");
        println!("               contacts.");
        println!("- MATES_DIR:   The vdir to use.");
        println!("- MATES_GREP:  The grep executable to use.");
    };

    if matches.opt_present("h") {
        print_usage();

    } else if matches.opt_present("index") {
        let index_file = expect_env(&env, "MATES_INDEX");
        let mates_dir = expect_env(&env, "MATES_DIR");
        println!("Rebuilding index file \"{}\"...", index_file);
        main_try!(build_index(
            &Path::new(index_file.as_slice()),
            &Path::new(mates_dir.as_slice())
        ), "Failed to build index");

    } else if matches.opt_present("mutt-search") {
        let index_file = expect_env(&env, "MATES_INDEX");
        let default_grep = "grep".into_string();
        let grep_cmd = match from_env(&env, "MATES_GREP") {
            Some(x) => x,
            None => &default_grep
        };

        // FIXME: Better way to write this? We already checked for presence of mutt-search before
        let query = matches.opt_str("mutt-search").expect("This should never happen and yet it did.");
        let mut cmd = io::Command::new(grep_cmd.as_slice());
        cmd.arg(query.as_slice());
        cmd.arg(index_file.as_slice());
        cmd.stdout(io::process::InheritFd(1));
        cmd.stderr(io::process::InheritFd(2));

        let cmd_error = format!("Failed to execute `{}`", cmd);
        let mut process = main_try!(cmd.spawn(), cmd_error);
        let code = main_try!(process.wait(), cmd_error);
        os::set_exit_status(match code {
            io::process::ExitStatus(x) => x,
            io::process::ExitSignal(x) => x,
        });

    } else {
        print_usage();
    };
}
