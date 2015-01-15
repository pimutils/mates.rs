use std::os;
use std::collections::HashMap;
use std::io;
use std::io::fs::PathExtensions;
use std::borrow::ToOwned;

use vobject::parse_component;

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


fn expect_env<'a>(env: &'a HashMap<String, String>, key: &str) -> &'a String {
    env.get(key).expect(
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

        print!("Processing {}\n", entry.display());

        let itemstr = try!(io::File::open(entry).read_to_string());
        let item = match parse_component(itemstr.as_slice()) {
            Ok(item) => item,
            Err(e) => {
                println!("Error: Failed to parse item {}: {}\n", entry.display(), e);
                os::set_exit_status(1);
                continue;
            }
        };

        let name = match item.single_prop("FN") {
            Some(name) => name.value_as_string(),
            None => {
                print!("Warning: No name in {}, skipping.\n", entry.display());
                continue;
            }
        };

        let emails = item.all_props("EMAIL");
        for email in emails.iter() {
            try!(outf.write_str(
                format!("{}\t{}\t{}\n", email.value_as_string(), name, entry.display()).as_slice()
            ))
        };
    };
    return Ok(());
}


pub fn cli_main() {
    let env = get_env();
    let mut args = os::args().into_iter();
    let program = args.next().unwrap_or("mates".to_string());

    let help = format!("Usage: {} COMMAND
Commands:
    index:
        Rewrite/create the index.
    mutt-query <query>:
        Search for contact, output is usable for mutt's query_command.
    file-query <query>:
        Search for contact, return just the filename.
    email-query <query>:
        Search for contact, return \"name <email>\".
    add:
        Take mail from stdin, add sender to contacts. Print filename.
    edit <file-or-query>:
        Open contact (given by filepath or search-string) in $MATES_EDITOR. If
        the file is cleared, the contact is removed.", program);

    let print_help = |&:| {
        println!("{}", help);
        println!("Environment variables:");
        println!("- MATES_INDEX: Path to index file, which is basically a cache of all");
        println!("               contacts.");
        println!("- MATES_DIR:   The vdir to use.");
        println!("- MATES_GREP:  The grep executable to use.");
    };

    let command = args.next().unwrap_or("".to_string());

    match command.as_slice() {
        "index" => {
            let index_file = expect_env(&env, "MATES_INDEX");
            let mates_dir = expect_env(&env, "MATES_DIR");
            println!("Rebuilding index file \"{}\"...", index_file);
            main_try!(build_index(
                &Path::new(index_file.as_slice()),
                &Path::new(mates_dir.as_slice())
            ), "Failed to build index");
        },
        "mutt-query" => {
            let query = args.next().unwrap_or("".to_string());
            main_try!(mutt_query(env, query), "Failed to execute grep");
        },
        "file-query" => {
            let query = args.next().unwrap_or("".to_string());
            main_try!(file_query(env, query), "Failed to execute grep");
        },
        "email-query" => {
            let query = args.next().unwrap_or("".to_string());
            main_try!(email_query(env, query), "Failed to execute grep");
        },
        _ => {
            print_help();
            if command != "help" && command != "--help" && command != "-h" {
                os::set_exit_status(1);
            }
        }
    };
}

fn mutt_query<'a>(env: HashMap<String, String>, query: String) -> io::IoResult<()> {
    println!("");  // For some reason mutt requires an empty line
    for item in try!(index_query(env, query)) {
        if item.email.len() > 0 && item.name.len() > 0 {
            println!("{}\t{}\t{}", item.email, item.name, item.filepath);
        };
    };
    Ok(())
}

fn file_query<'a>(env: HashMap<String, String>, query: String) -> io::IoResult<()> {
    for item in try!(index_query(env, query)) {
        if item.filepath.len() > 0 {
            println!("{}", item.filepath)
        };
    };
    Ok(())
}

fn email_query<'a>(env: HashMap<String, String>, query: String) -> io::IoResult<()> {
    for item in try!(index_query(env, query)) {
        if item.name.len() > 0 && item.email.len() > 0 {
            println!("{} <{}>", item.name, item.email)
        };
    };
    Ok(())
}

fn index_query<'a>(env: HashMap<String, String>, query: String) -> io::IoResult<IndexIterator<'a>> {
    let default_grep = "grep".to_owned();
    let grep_cmd = match env.get("MATES_GREP") {
        Some(x) => x,
        None => &default_grep
    };

    let index_path = Path::new(expect_env(&env, "MATES_INDEX"));
    let mut process = try!(io::Command::new(grep_cmd.as_slice())
        .arg(query.as_slice())
        .stderr(io::process::InheritFd(2))
        .spawn());

    {
        let mut index_fp = try!(io::File::open(&index_path));
        let mut stdin = process.stdin.take().unwrap();
        try!(stdin.write_str(try!(index_fp.read_to_string()).as_slice()));
    }

    let stream = match process.stdout.as_mut() {
        Some(x) => x,
        None => return Err(io::IoError {
            kind: io::IoUnavailable,
            desc: "Failed to get stdout from grep process.",
            detail: None
        })
    };

    let output = try!(stream.read_to_string());
    Ok(IndexIterator::new(&output))
}

struct IndexItem<'a> {
    pub email: String,
    pub name: String,
    pub filepath: String
}

impl<'a> IndexItem<'a> {
    fn new(line: String) -> IndexItem<'a> {
        let mut parts = line.split('\t');

        IndexItem {
            email: parts.next().unwrap_or("").to_string(),
            name: parts.next().unwrap_or("").to_string(),
            filepath: parts.next().unwrap_or("").to_string()
        }
    }
}

struct IndexIterator<'a> {
    linebuffer: Vec<String>
}

impl<'a> IndexIterator<'a> {
    fn new(output: &String) -> IndexIterator<'a> {

        let rv = output.split('\n').map(|x: &str| x.to_string()).collect();
        IndexIterator {
            linebuffer: rv
        }
    }
}

impl<'a> Iterator for IndexIterator<'a> {
    type Item = IndexItem<'a>;

    fn next(&mut self) -> Option<IndexItem<'a>> {
        match self.linebuffer.pop() {
            Some(x) => Some(IndexItem::new(x)),
            None => None
        }
    }
}
