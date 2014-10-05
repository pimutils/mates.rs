#![feature(macro_rules)]

extern crate getopts;
use getopts::{optflag,optopt,getopts,usage};
use std::os;
use std::collections::HashMap;
use std::collections::hashmap::{Occupied, Vacant};
use std::io;
use std::io::fs::PathExtensions;


fn get_env() -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.extend(os::env().into_iter().filter(|&(ref key, ref value)| {
        key.as_slice().starts_with("MATES_")
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


macro_rules! expect_or_this(
    ($iterator:expr, $this: expr) => (
        match $iterator {
            Some(x) => x,
            None => $this
        }
    )
)


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
        let name = item.fullname();
        match item.emails() {
            Some(emails) => {
                for email in emails.iter() {
                    outf.write_str(format!("{}\t{}\n", email.value, name).as_slice());
                };
            },
            None => ()
        };
    };
}

struct PropertyValue {
    params: String,
    value: String,
}


struct Item {
    props: HashMap<String, Vec<PropertyValue>>
}

impl Item {
    fn single_value(&self, key: &String) -> Option<String> {
        match self.props.find(key) {
            Some(x) => { if x.len() > 0 { Some(x[0].value.clone()) } else { None } },
            None => { None }
        }
    }
    fn fullname(&self) -> String {
        match self.single_value(&"FN".into_string()) {
            Some(x) => { x }
            None => { "".into_string() }
        }
    }

    fn emails(&self) -> Option<&Vec<PropertyValue>> {
        self.props.find(&"EMAIL".into_string())
    }
}


fn parse_item(s: &String) -> Item {
    let mut linebuffer = String::new();
    let mut line: String;
    let mut is_continuation: bool;
    let mut rv = Item {
        props: HashMap::new()
    };

    for strline in s.as_slice().split('\n') {
        line = strline.into_string();
        is_continuation = false;
        while line.as_slice().char_at(0).is_whitespace() {
            is_continuation = true;
            line.remove(0);
        };

        if !is_continuation && linebuffer.len() > 0 {
            let (propkey, propvalue) = parse_line(&linebuffer);
            match rv.props.entry(propkey) {
                Occupied(values) => { values.into_mut().push(propvalue); },
                Vacant(values) => { values.set(vec![propvalue]); }
            };
            linebuffer.clear();
        };

        linebuffer.push_str(line.as_slice());
    };
    rv
}


fn parse_line(s: &String) -> (String, PropertyValue) {
    let mut kv_splitresult = s.as_slice().splitn(1, ':');
    let key_and_params = kv_splitresult.next().expect("");
    let value = expect_or_this!(kv_splitresult.next(), "");

    let mut kp_splitresult = key_and_params.splitn(1, ';');
    let key = kp_splitresult.next().expect("");
    let params = expect_or_this!(kp_splitresult.next(), "");

    (key.into_string(), PropertyValue {
        value: value.into_string(),
        params: params.into_string()
    })
}


fn main() {
    let args = os::args();

    let program = args[0].as_slice();
    let opts = [
        optflag("i", "index", "Create index."),
        optflag("h", "help", "Print help."),
        optopt("m", "mutt-search", "Search in index, for mutt search.", "")
    ];

    let matches = match getopts(args.tail(), opts) {
        Ok(m) => { m }
        Err(f) => { fail!(f.to_err_msg()) }
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
        let grep_cmd = expect_or_this!(from_env(&env, "MATES_GREP"), &default_grep);

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
                    if !code.success() {
                        fail!("Grep command exited with code {}, aborting.", code);
                    };
                },
                Err(e) => fail!("Failed to execute grep command: {}", e),
            };
    } else {
        print_usage();
    };
}
