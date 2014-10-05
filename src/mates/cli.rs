use getopts::{optflag,optopt,getopts,usage};
use std::os;
use std::collections::HashMap;
use std::io;
use std::io::fs::PathExtensions;

use item::parse_item;

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


fn build_index(outfile: &Path, dir: &Path) {
    if !dir.is_dir() {
        fail!("MATES_DIR must be a directory.");
    };

    let mut outf = io::File::create(outfile);
    let entries = match io::fs::readdir(dir) {
        Ok(x) => x,
        Err(f) => { fail!(f.desc) }
    };
    for entry in entries.iter() {
        if !entry.is_file() {
            continue;
        }
        let itemstr = match io::File::open(entry).read_to_string() {
            Ok(x) => x,
            Err(f) => { fail!(format!("Failed to open {}: {}", entry.display(), f.desc)) }
        };
        let item = parse_item(&itemstr);
        let name = item.single_value(&"FN".into_string());
        match item.all_values(&"EMAIL".into_string()) {
            Some(emails) => {
                for email in emails.iter() {
                    outf.write_str(format!("{}\t{}\n", email.get_raw_value(), name).as_slice());
                };
            },
            None => ()
        };
    };
}


pub fn cli_main() {
    let args = os::args();

    let program = args[0].as_slice();
    let opts = [
        optflag("i", "index", "Create index."),
        optflag("h", "help", "Print help."),
        optopt("m", "mutt-search", "Search in index, for mutt search.", "")
    ];

    let matches = match getopts(args.tail(), opts) {
        Ok(m) => { m }
        Err(f) => { fail!(format!("Failed to parse arguments: {}", f.to_err_msg())) }
    };

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
        build_index(
            &Path::new(index_file.as_slice()),
            &Path::new(mates_dir.as_slice())
        );
    } else if matches.opt_present("mutt-search") {
        let index_file = expect_env(&env, "MATES_INDEX");
        let default_grep = "grep".into_string();
        let grep_cmd = match from_env(&env, "MATES_GREP") {
            Some(x) => x,
            None => &default_grep
        };

        // FIXME: Better way to write this? We already checked for presence of mutt-search before
        let query = matches.opt_str("mutt-search").expect("This should never happen and yet it did.");
        match match io::Command::new(grep_cmd.as_slice())
            .arg(query.as_slice())
            .arg(index_file.as_slice())
            .stdout(io::process::InheritFd(1))
            .stderr(io::process::InheritFd(2))
            .spawn() {
                Ok(child) => child,
                Err(e) => fail!("Failed to execute grep command: {}", e),
            }.wait() {
                Ok(code) => {
                    os::set_exit_status(match code {
                        io::process::ExitStatus(x) => x,
                        io::process::ExitSignal(x) => x,
                    });
                },
                Err(e) => fail!("Failed to execute grep command: {}", e),
            };
    } else {
        print_usage();
    };
}
