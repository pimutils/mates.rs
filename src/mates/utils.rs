use std::borrow::ToOwned;
use std::collections::HashSet;
use std::ffi::AsOsStr;
use std::fs::PathExt;
use std::fs;
use std::io::{Read,Write};
use std::io;
use std::path::AsPath;
use std::path;
use std::process;

use atomicwrites::{GenericAtomicFile,AtomicFile,DisallowOverwrite};
use email::rfc5322::Rfc5322Parser;
use uuid::Uuid;
use vobject::{Component,Property,parse_component,write_component};

use cli::Configuration;

pub fn handle_process(process: &mut process::Child) -> io::Result<()> {
    let exitcode = try!(process.wait());
    if !exitcode.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "",
            Some(format!("{}", exitcode))
        ));
    };
    Ok(())
}


struct IndexIterator {
    linebuffer: Vec<String>
}

impl IndexIterator {
    fn new(output: &String) -> IndexIterator {
        let rv = output.split('\n').map(|x| x.to_string()).collect();
        IndexIterator {
            linebuffer: rv
        }
    }
}

impl Iterator for IndexIterator {
    type Item = IndexItem;

    fn next(&mut self) -> Option<IndexItem> {
        match self.linebuffer.pop() {
            Some(x) => Some(IndexItem::new(x)),
            None => None
        }
    }
}

struct IndexItem {
    pub email: String,
    pub name: String,
    pub filepath: Option<path::PathBuf>
}

impl IndexItem {
    fn new(line: String) -> IndexItem {
        let mut parts = line.split('\t');

        IndexItem {
            email: parts.next().unwrap_or("").to_string(),
            name: parts.next().unwrap_or("").to_string(),
            filepath: match parts.next() {
                Some(x) => Some(path::PathBuf::new(x)),
                None => None
            }
        }
    }
}

pub struct Contact {
    pub component: Component,
    pub path: path::PathBuf
}

impl Contact {
    pub fn from_file<P: AsOsStr + path::AsPath + ?Sized>(path: &P) -> io::Result<Contact> {
        // FIXME: Why is "AsOsStr" above necessary? File::open has a sig w/o it
        let mut contact_file = try!(fs::File::open(&path));
        let contact_string = {
            let mut x = String::new();
            try!(contact_file.read_to_string(&mut x));
            x
        };

        let item = match parse_component(contact_string.as_slice()) {
            Ok(x) => x,
            Err(e) => return Err(io::Error::new(
                io::ErrorKind::Other,
                "Error while parsing contact",
                Some(e)
            ))
        };

        Ok(Contact { component: item, path: path.as_path().to_owned() })
    }

    pub fn generate(fullname: Option<&str>, email: Option<&str>, dir: &path::Path) -> Contact {
        let (uid, contact_path) = {
            let mut uid;
            let mut contact_path;
            loop {
                uid = Uuid::new_v4().to_simple_string();
                contact_path = dir.join(&format!("{}.vcf", uid));
                if !(*contact_path).exists() {
                    break
                }
            };
            (uid, contact_path)
        };
        Contact { path: contact_path, component: generate_component(uid, fullname, email) }
    }

    pub fn write_create(&self) -> io::Result<()> {
        let string = write_component(&self.component);
        let af: AtomicFile = GenericAtomicFile::new(&self.path, DisallowOverwrite);

        af.write(|f| {
            f.write_all(string.as_bytes())
        })
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

pub fn index_query<'a>(config: &Configuration, query: &str) -> io::Result<IndexIterator> {
    let mut process = try!(
        command_from_config(config.grep_cmd.as_slice())
        .arg(query.as_slice())
        .stderr(process::Stdio::inherit())
        .spawn());

    {
        let mut index_fp = try!(fs::File::open(&config.index_path));
        let mut stdin = process.stdin.take().unwrap();
        let mut line: Vec<u8> = vec![];
        try!(index_fp.read_to_end(&mut line));
        try!(stdin.write_all(line.as_slice()));
    }

    try!(handle_process(&mut process));

    let stream = match process.stdout.as_mut() {
        Some(x) => x,
        None => return Err(io::Error::new(
            io::ErrorKind::ResourceUnavailable,
            "Failed to get stdout from grep process.",
            None
        ))
    };

    let mut output = String::new();
    try!(stream.read_to_string(&mut output));
    Ok(IndexIterator::new(&output))
}

/// Better than index_query if you're only interested in the filepath, as duplicate entries will be
/// removed.
pub fn file_query(config: &Configuration, query: &str) -> io::Result<HashSet<path::PathBuf>> {
    let mut rv: HashSet<path::PathBuf> = HashSet::new();
    rv.extend(
        try!(index_query(config, query)).filter_map(|x| x.filepath)
    );
    Ok(rv)
}

pub fn index_item_from_contact(contact: &Contact) -> io::Result<String> {
    let name = match contact.component.single_prop("FN") {
        Some(name) => name.value_as_string(),
        None => return Err(io::Error::new(
            io::ErrorKind::Other,
            "No name found.",
            None
        ))
    };

    let emails = contact.component.all_props("EMAIL");
    let mut rv = String::new();
    for email in emails.iter() {
        rv.push_str(format!("{}\t{}\t{}\n", email.value_as_string(), name, contact.path.display()).as_slice());
    };
    Ok(rv)
}

/// Return a tuple (fullname, email)
pub fn parse_from_header<'a>(s: &'a String) -> (Option<&'a str>, Option<&'a str>) {
    let mut split = s.rsplitn(1, ' ');
    let email = match split.next() {
        Some(x) => Some(x.trim_left_matches('<').trim_right_matches('>')),
        None => Some(s.as_slice())
    };
    let name = split.next();
    (name, email)
}

/// Given an email, return value of From header.
pub fn read_sender_from_email(email: &str) -> Option<String> {
    let mut parser = Rfc5322Parser::new(email);
    while !parser.eof() {
        match parser.consume_header() {
            Some(header) => {
                if header.name == "From" {
                    return header.get_value().ok()
                };
            },
            None => return None
        };
    };
    None
}

/// Write sender from given email as .vcf file to given directory.
pub fn add_contact_from_email(contact_dir: &path::Path, email_input: &str) -> io::Result<Contact> {
    let from_header = match read_sender_from_email(email_input) {
        Some(x) => x,
        None => return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Couldn't find From-header in email.",
            None
        ))
    };
    let (fullname, email) = parse_from_header(&from_header);
    let contact = Contact::generate(fullname, email, contact_dir);
    try!(contact.write_create());
    Ok(contact)
}


fn command_from_config(config_val: &str) -> process::Command {
    let mut parts = config_val.split(' ');
    let main = parts.next().unwrap();
    let rest: Vec<_> = parts.map(|x| x.to_string()).collect();
    let mut rv = process::Command::new(main);
    rv.args(rest.as_slice());
    rv
}
