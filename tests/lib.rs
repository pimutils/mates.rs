#![feature(globs)]
extern crate mates;
use std::collections::HashMap;
use mates::item::parse_item_from_borrowed_string;

#[test]
fn test_wikipedia_1() {
    let item = parse_item_from_borrowed_string(
        "BEGIN:VCARD\n\
        VERSION:2.1\n\
        N:Mustermann;Erika\n\
        FN:Erika Mustermann\n\
        ORG:Wikipedia\n\
        TITLE:Oberleutnant\n\
        PHOTO;JPEG:http://commons.wikimedia.org/wiki/File:Erika_Mustermann_2010.jpg\n\
        TEL;WORK;VOICE:(0221) 9999123\n\
        TEL;HOME;VOICE:(0221) 1234567\n\
        ADR;HOME:;;Heidestrasse 17;Koeln;;51147;Deutschland\n\
        EMAIL;PREF;INTERNET:erika@mustermann.de\n\
        REV:20140301T221110Z\n\
        END:VCARD").unwrap();

    assert_eq!(item.single_value(&"FN".into_string()), Some(&"Erika Mustermann".into_string()));
    assert_eq!(item.single_value(&"N".into_string()), Some(&"Mustermann;Erika".into_string()));
}
