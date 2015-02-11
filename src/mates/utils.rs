use std::old_io;
use std::old_io::fs::PathExtensions;
use std::collections::HashSet;

use vobject::{Component,Property,parse_component,write_component};
use email::rfc5322::Rfc5322Parser;
use uuid::Uuid;
use atomicwrites::{GenericAtomicFile,AtomicFile,DisallowOverwrite};

use cli::Configuration;

struct IndexIterator<'a> {
    linebuffer: Vec<String>
}

impl<'a> IndexIterator<'a> {
    fn new(output: &String) -> IndexIterator<'a> {
        let rv = output.split('\n').map(|x| x.to_string()).collect();
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

struct IndexItem<'a> {
    pub email: String,
    pub name: String,
    pub filepath: Option<Path>
}

impl<'a> IndexItem<'a> {
    fn new(line: String) -> IndexItem<'a> {
        let mut parts = line.split('\t');

        IndexItem {
            email: parts.next().unwrap_or("").to_string(),
            name: parts.next().unwrap_or("").to_string(),
            filepath: match parts.next() {
                Some(x) => Some(Path::new(x)),
                None => None
            }
        }
    }
}

pub struct Contact {
    pub component: Component,
    pub path: Path
}

impl Contact {
    pub fn from_file(path: Path) -> old_io::IoResult<Contact> {
        let mut contact_file = try!(old_io::File::open(&path));
        let contact_string = try!(contact_file.read_to_string());
        let item = match parse_component(contact_string.as_slice()) {
            Ok(x) => x,
            Err(e) => return Err(old_io::IoError {
                kind: old_io::OtherIoError,
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

    pub fn write_create(&self) -> old_io::IoResult<()> {
        let string = write_component(&self.component);
        let af: AtomicFile = GenericAtomicFile::new(&self.path, DisallowOverwrite);

        af.write(|&: f| {
            f.write_str(string.as_slice())
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

pub fn index_query<'a>(config: &Configuration, query: &str) -> old_io::IoResult<IndexIterator<'a>> {
    let mut process = try!(old_io::Command::new(config.grep_cmd.as_slice())
        .arg(query.as_slice())
        .stderr(old_io::process::InheritFd(2))
        .spawn());

    {
        let mut index_fp = try!(old_io::File::open(&config.index_path));
        let mut stdin = process.stdin.take().unwrap();
        try!(stdin.write_str(try!(index_fp.read_to_string()).as_slice()));
    }

    let stream = match process.stdout.as_mut() {
        Some(x) => x,
        None => return Err(old_io::IoError {
            kind: old_io::IoUnavailable,
            desc: "Failed to get stdout from grep process.",
            detail: None
        })
    };

    let output = try!(stream.read_to_string());
    Ok(IndexIterator::new(&output))
}

/// Better than index_query if you're only interested in the filepath, as duplicate entries will be
/// removed.
pub fn file_query(config: &Configuration, query: &str) -> old_io::IoResult<HashSet<Path>> {
    let mut rv: HashSet<Path> = HashSet::new();
    rv.extend(
        try!(index_query(config, query)).filter_map(|x| x.filepath)
    );
    Ok(rv)
}

pub fn index_item_from_contact(contact: &Contact) -> old_io::IoResult<String> {
    let name = match contact.component.single_prop("FN") {
        Some(name) => name.value_as_string(),
        None => return Err(old_io::IoError {
            kind: old_io::OtherIoError,
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
                    return header.get_value()
                };
            },
            None => return None
        };
    };
    None
}

/// Write sender from given email as .vcf file to given directory.
pub fn add_contact_from_email(contact_dir: &Path, email_input: &str) -> old_io::IoResult<Contact> {
    let from_header = match read_sender_from_email(email_input) {
        Some(x) => x,
        None => return Err(old_io::IoError {
            kind: old_io::InvalidInput,
            desc: "Couldn't find From-header in email.",
            detail: None
        })
    };
    let (fullname, email) = parse_from_header(&from_header);
    let contact = Contact::generate(fullname, email, contact_dir);
    try!(contact.write_create());
    Ok(contact)
}
