use std::os;
use std::collections::HashMap;
use std::io;
use std::io::fs::PathExtensions;
use std::borrow::ToOwned;

use vobject::{Component,Property,parse_component,write_component};
use email::rfc5322::Rfc5322Parser;
use uuid::Uuid;

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
        (key.as_slice() == "EDITOR" || key.as_slice().starts_with("MATES_")) &&
            value.len() > 0
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
    let mut errors = false;

    for entry in entries.iter() {
        if !entry.is_file() {
            continue;
        }

        let contact = match Contact::from_file(entry.clone()) {
            Ok(x) => x,
            Err(e) => {
                println!("Error while reading {}: {}", entry.display(), e);
                errors = true;
                continue
            }
        };

        match index_item_from_contact(&contact) {
            Ok(index_string) => {
                try!(outf.write_str(index_string.as_slice()));
            },
            Err(e) => {
                println!("Error while indexing {}: {}", entry.display(), e);
                errors = true;
                continue
            }
        };
    };

    if errors {
        Err(io::IoError {
            kind: io::OtherIoError,
            desc: "Several errors happened while generating the index.",
            detail: None
        })
    } else {
        Ok(())
    }
}


fn index_item_from_contact(contact: &Contact) -> io::IoResult<String> {
    let name = match contact.component.single_prop("FN") {
        Some(name) => name.value_as_string(),
        None => return Err(io::IoError {
            kind: io::OtherIoError,
            desc: "No name found.",
            detail: None
        })
    };

    let emails = contact.component.all_props("EMAIL");
    let mut rv = String::new();
    for email in emails.iter() {
        rv.push_str(format!("{}\t{}\t{}\n", email.value_as_string(), name, contact.path.display()).as_slice());
    };
    Ok(rv)
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
            main_try!(mutt_query(&env, query.as_slice()), "Failed to execute grep");
        },
        "file-query" => {
            let query = args.next().unwrap_or("".to_string());
            main_try!(file_query(&env, query.as_slice()), "Failed to execute grep");
        },
        "email-query" => {
            let query = args.next().unwrap_or("".to_string());
            main_try!(email_query(&env, query.as_slice()), "Failed to execute grep");
        },
        "add" => {
            let index_file = Path::new(expect_env(&env, "MATES_INDEX"));
            let mates_dir = Path::new(expect_env(&env, "MATES_DIR"));
            let contact = main_try!(add_contact(&mates_dir), "Failed to add contact");
            println!("{}", contact.path.display());

            let mut index_fp = main_try!(io::File::open_mode(
                &index_file,
                io::Append,
                io::Write),
                "Failed to open index"
            );

            let index_entry = main_try!(index_item_from_contact(&contact), "Failed to generate index");
            main_try!(index_fp.write_str(index_entry.as_slice()), "Failed to write to index");
        },
        "edit" => {
            let query = args.next().unwrap_or("".to_string());
            let mates_dir = expect_env(&env, "MATES_DIR");
            main_try!(edit_contact(&env, query.as_slice(), mates_dir.as_slice()),
                      "Failed to edit contact");
        },
        _ => {
            print_help();
            if command != "help" && command != "--help" && command != "-h" {
                os::set_exit_status(1);
            }
        }
    };
}

fn add_contact(contact_dir: &Path) -> io::IoResult<Contact> {
    let stdin = try!(io::stdin().lock().read_to_string());
    let from_header = match read_sender_from_email(stdin.as_slice()) {
        Some(x) => x,
        None => return Err(io::IoError {
            kind: io::InvalidInput,
            desc: "Couldn't find From-header in email.",
            detail: None
        })
    };
    let (fullname, email) = parse_from_header(&from_header);
    let contact = Contact::generate(fullname, email, contact_dir);
    try!(contact.write_create());
    Ok(contact)
}


/// Return a tuple (fullname, email)
fn parse_from_header<'a>(s: &'a String) -> (Option<&'a str>, Option<&'a str>) {
    let mut split = s.rsplitn(1, ' ');
    let email = match split.next() {
        Some(x) => Some(x.trim_left_matches('<').trim_right_matches('>')),
        None => Some(s.as_slice())
    };
    let name = split.next();
    (name, email)
}

/// Given an email, return value of From header.
fn read_sender_from_email(email: &str) -> Option<String> {
    let mut parser = Rfc5322Parser::new(email);
    while !parser.eof() {
        match parser.consume_header() {
            Some(header) => {
                if header.name == "From" {
                    return header.get_value()
                };
            },
            None => return None
        };
    };
    None
}

fn edit_contact(env: &HashMap<String, String>, query: &str, mates_dir: &str) -> Result<(), String> {
    let editor_cmd = match env.get("MATES_EDITOR") {
        Some(x) => x.as_slice(),
        None => match env.get("EDITOR") {
            Some(x) => x.as_slice(),
            None => return Err("Either MATES_EDITOR or EDITOR has to be set.".to_string())
        }
    };

    let results = {
        if Path::new(mates_dir).join(query).exists() {
            vec![query.to_string()]
        } else {
            let results_iter = match index_query(env, query) {
                Ok(x) => x,
                Err(e) => return Err(format!("Error while fetching index: {}", e))
            };

            results_iter.filter_map(|x| {
                if x.filepath.len() > 0 {
                    Some(x.filepath)
                } else {
                    None
                }}).collect()
        }
    };

    if results.len() < 1 {
        return Err("No such contact.".to_string());
    } else if results.len() > 1 {
        for fname in results.iter() {
            println!("{}", fname);
        };
        return Err("Ambiguous query.".to_string());
    }

    let fpath = results[0].as_slice();
    let mut process = match io::Command::new("sh")
        .arg("-c")
        // clear stdin, http://unix.stackexchange.com/a/77593
        .arg(format!("$0 -- \"$1\" < $2"))
        .arg(editor_cmd.as_slice())
        .arg(fpath)
        .arg("/dev/tty")
        .stdin(io::process::InheritFd(0))
        .stdout(io::process::InheritFd(1))
        .stderr(io::process::InheritFd(2))
        .spawn() {
            Ok(x) => x,
            Err(e) => return Err(format!("Error while invoking editor: {}", e))
        };

    match process.wait() {
        Ok(_) => (),
        Err(e) => return Err(format!("Error while invoking editor: {}", e))
    };

    if match io::File::open(&Path::new(fpath)).read_to_string() {
        Ok(x) => x,
        Err(e) => return Err(format!("File can't be read after user edited it: {}", e))
    }.as_slice().trim().len() == 0 {
        return Err(format!("Contact emptied, file removed."));
    };

    Ok(())
}

fn mutt_query<'a>(env: &HashMap<String, String>, query: &str) -> io::IoResult<()> {
    println!("");  // For some reason mutt requires an empty line
    for item in try!(index_query(env, query)) {
        if item.email.len() > 0 && item.name.len() > 0 {
            println!("{}\t{}\t{}", item.email, item.name, item.filepath);
        };
    };
    Ok(())
}

fn file_query<'a>(env: &HashMap<String, String>, query: &str) -> io::IoResult<()> {
    for item in try!(index_query(env, query)) {
        if item.filepath.len() > 0 {
            println!("{}", item.filepath)
        };
    };
    Ok(())
}

fn email_query<'a>(env: &HashMap<String, String>, query: &str) -> io::IoResult<()> {
    for item in try!(index_query(env, query)) {
        if item.name.len() > 0 && item.email.len() > 0 {
            println!("{} <{}>", item.name, item.email);
        };
    };
    Ok(())
}

fn index_query<'a>(env: &HashMap<String, String>, query: &str) -> io::IoResult<IndexIterator<'a>> {
    let default_grep = "grep".to_owned();
    let grep_cmd = match env.get("MATES_GREP") {
        Some(x) => x,
        None => &default_grep
    };

    let index_path = Path::new(expect_env(env, "MATES_INDEX"));
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

struct Contact {
    pub component: Component,
    pub path: Path
}

impl Contact {
    pub fn from_file(path: Path) -> io::IoResult<Contact> {
        let mut contact_file = try!(io::File::open(&path));
        let contact_string = try!(contact_file.read_to_string());
        let item = match parse_component(contact_string.as_slice()) {
            Ok(x) => x,
            Err(e) => return Err(io::IoError {
                kind: io::OtherIoError,
                desc: "Error while parsing contact",
                detail: Some(e)
            })
        };
        Ok(Contact { component: item, path: path })
    }

    pub fn generate(fullname: Option<&str>, email: Option<&str>, dir: &Path) -> Contact {
        let (uid, contact_path) = {
            let mut uid;
            let mut contact_path;
            loop {
                uid = Uuid::new_v4().to_simple_string();
                contact_path = dir.join(Path::new(format!("{}.vcf", uid)));
                if !contact_path.exists() {
                    break
                }
            };
            (uid, contact_path)
        };
        Contact { path: contact_path, component: generate_component(uid, fullname, email) }
    }

    pub fn write_create(&self) -> io::IoResult<()> {
        let string = write_component(&self.component);
        let mut fp = try!(io::File::create(&self.path));
        fp.write_str(string.as_slice())
    }
}


fn generate_component(uid: String, fullname: Option<&str>, email: Option<&str>) -> Component {
    let mut comp = Component::new("VCARD");

    match fullname {
        Some(x) => comp.all_props_mut("FN").push(Property::new(x)),
        None => ()
    };

    match email {
        Some(x) => comp.all_props_mut("EMAIL").push(Property::new(x)),
        None => ()
    };
    comp.all_props_mut("UID").push(Property::new(uid.as_slice()));
    comp
}
