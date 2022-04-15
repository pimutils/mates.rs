use std::borrow::ToOwned;
use std::collections::HashSet;
use std::convert::AsRef;
use std::fs;
use std::io;
use std::io::{Read, Write};
use std::path;
use std::process;

use atomicwrites::{AtomicFile, DisallowOverwrite};
use email::rfc5322::Rfc5322Parser;
use uuid::Uuid;
use vobject::{parse_component, write_component, Component, Property};

use crate::cli::Configuration;

pub trait CustomPathExt {
    fn metadata(&self) -> io::Result<fs::Metadata>;
    fn exists(&self) -> bool;
    fn is_file(&self) -> bool;
    fn is_dir(&self) -> bool;
    fn str_extension(&self) -> Option<&str>;
}

impl CustomPathExt for path::Path {
    fn metadata(&self) -> io::Result<fs::Metadata> {
        fs::metadata(self)
    }

    fn exists(&self) -> bool {
        fs::metadata(self).is_ok()
    }

    fn is_file(&self) -> bool {
        fs::metadata(self).map(|s| s.is_file()).unwrap_or(false)
    }
    fn is_dir(&self) -> bool {
        fs::metadata(self).map(|s| s.is_dir()).unwrap_or(false)
    }

    fn str_extension(&self) -> Option<&str> {
        self.extension().and_then(|x| x.to_str())
    }
}

pub fn handle_process(process: &mut process::Child) -> io::Result<()> {
    let exitcode = process.wait()?;
    if !exitcode.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("{}", exitcode),
        ));
    };
    Ok(())
}

pub struct IndexIterator {
    linebuffer: Vec<String>,
}

impl IndexIterator {
    fn new(output: &String) -> IndexIterator {
        let rv = output.split('\n').map(|x| x.to_string()).collect();
        IndexIterator { linebuffer: rv }
    }
}

impl Iterator for IndexIterator {
    type Item = IndexItem;

    fn next(&mut self) -> Option<IndexItem> {
        match self.linebuffer.pop() {
            Some(x) => Some(IndexItem::new(x)),
            None => None,
        }
    }
}

pub struct IndexItem {
    pub email: String,
    pub name: String,
    pub filepath: Option<path::PathBuf>,
}

impl IndexItem {
    fn new(line: String) -> IndexItem {
        let mut parts = line.split('\t');

        IndexItem {
            email: parts.next().unwrap_or("").to_string(),
            name: parts.next().unwrap_or("").to_string(),
            filepath: match parts.next() {
                Some(x) => Some(path::PathBuf::from(x)),
                None => None,
            },
        }
    }
}

pub struct Contact {
    pub component: Component,
    pub path: path::PathBuf,
}

impl Contact {
    pub fn from_file<P: AsRef<path::Path>>(path: P) -> io::Result<Contact> {
        let mut contact_file = fs::File::open(&path)?;
        let contact_string = {
            let mut x = String::new();
            contact_file.read_to_string(&mut x)?;
            x
        };

        let item = match parse_component(&contact_string[..]) {
            Ok(x) => x,
            Err(e) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Error while parsing contact: {}", e),
                ))
            }
        };

        Ok(Contact {
            component: item,
            path: path.as_ref().to_owned(),
        })
    }

    pub fn generate(fullname: Option<&str>, email: Option<&str>, dir: &path::Path) -> Contact {
        let (uid, contact_path) = {
            let mut uid;
            let mut contact_path;
            loop {
                uid = Uuid::new_v4().hyphenated().to_string();
                contact_path = dir.join(&format!("{}.vcf", uid));
                if !(*contact_path).exists() {
                    break;
                }
            }
            (uid, contact_path)
        };
        Contact {
            path: contact_path,
            component: generate_component(uid.into(), fullname, email),
        }
    }

    pub fn write_create(&self) -> io::Result<()> {
        let string = write_component(&self.component);
        let af = AtomicFile::new(&self.path, DisallowOverwrite);

        af.write(|f| f.write_all(string.as_bytes()))?;
        Ok(())
    }
}

fn generate_component(uid: String, fullname: Option<&str>, email: Option<&str>) -> Component {
    let mut comp = Component::new("VCARD");

    comp.push(Property::new("VERSION", "3.0"));

    match fullname {
        Some(x) => comp.push(Property::new("FN", x)),
        None => (),
    };

    match email {
        Some(x) => comp.push(Property::new("EMAIL", x)),
        None => (),
    };
    comp.push(Property::new("UID", &uid[..]));
    comp
}

pub fn index_query<'a>(config: &Configuration, query: &str) -> io::Result<IndexIterator> {
    let mut process = command_from_config(&config.grep_cmd[..])
        .arg(&query[..])
        .arg(&config.index_path)
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::inherit())
        .spawn()?;

    handle_process(&mut process)?;

    let stream = match process.stdout.as_mut() {
        Some(x) => x,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to get stdout from grep process.",
            ))
        }
    };

    let mut output = String::new();
    stream.read_to_string(&mut output)?;
    Ok(IndexIterator::new(&output))
}

/// Better than index_query if you're only interested in the filepath, as duplicate entries will be
/// removed.
pub fn file_query(config: &Configuration, query: &str) -> io::Result<HashSet<path::PathBuf>> {
    let mut rv: HashSet<path::PathBuf> = HashSet::new();
    rv.extend(index_query(config, query)?.filter_map(|x| x.filepath));
    Ok(rv)
}

pub fn index_item_from_contact(contact: &Contact) -> io::Result<String> {
    let name = match contact.component.get_only("FN") {
        Some(name) => name.value_as_string(),
        None => return Err(io::Error::new(io::ErrorKind::Other, "No name found.")),
    };

    let emails = contact.component.get_all("EMAIL");
    let mut rv = String::new();
    for email in emails.iter() {
        rv.push_str(
            &format!(
                "{}\t{}\t{}\n",
                email.value_as_string(),
                name,
                contact.path.display()
            )[..],
        );
    }
    Ok(rv)
}

/// Return a tuple (fullname, email)
pub fn parse_from_header<'a>(s: &'a String) -> (Option<&'a str>, Option<&'a str>) {
    let mut split = s.rsplitn(2, '<');
    let email = match split.next() {
        Some(x) => Some(x.trim_end_matches('>')),
        None => Some(&s[..]),
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
                    return header.get_value().ok();
                };
            }
            None => return None,
        };
    }
    None
}

/// Write sender from given email as .vcf file to given directory.
pub fn add_contact_from_email(contact_dir: &path::Path, email_input: &str) -> io::Result<Contact> {
    let from_header = match read_sender_from_email(email_input) {
        Some(x) => x,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Couldn't find From-header in email.",
            ))
        }
    };
    let (fullname, email) = parse_from_header(&from_header);
    let contact = Contact::generate(fullname, email, contact_dir);
    contact.write_create()?;
    Ok(contact)
}

fn command_from_config(config_val: &str) -> process::Command {
    let mut parts = config_val.split(' ');
    let main = parts.next().unwrap();
    let rest: Vec<_> = parts.map(|x| x.to_string()).collect();
    let mut rv = process::Command::new(main);
    rv.args(&rest[..]);
    rv
}
