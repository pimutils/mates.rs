#[phase(plugin)]
extern crate peg_syntax_ext;

use std::collections::HashMap;
use std::collections::hash_map::{Occupied, Vacant};

pub struct PropertyValue {
    params: String,
    value: String,
}

impl PropertyValue {
    pub fn get_raw_value(&self) -> &String { &self.value }
    pub fn get_raw_params(&self) -> &String { &self.params }
}


pub struct Item {
    pub props: HashMap<String, Vec<PropertyValue>>
}

impl Item {
    fn new() -> Item {
        Item {
            props: HashMap::new()
        }
    }

    pub fn single_value(&self, key: &String) -> Option<&String> {
        match self.props.get(key) {
            Some(x) => { if x.len() > 0 { Some(x[0].get_raw_value()) } else { None } },
            None => { None }
        }
    }

    pub fn all_props_mut(&mut self, key: String) -> &mut Vec<PropertyValue> {
        match self.props.entry(key) {
            Occupied(values) => values.into_mut(),
            Vacant(values) => values.set(vec![])
        }
    }

    pub fn all_props(&self, key: &String) -> &[PropertyValue] {
        static EMPTY: &'static [PropertyValue] = [];
        match self.props.get(key) {
            Some(values) => values.as_slice(),
            None => EMPTY
        }
    }
}


peg! parser(r#"
use super::{Item,PropertyValue};

#[pub]

item -> Item
    = p:prop ++ eol {
        let mut rv = Item::new();

        for (k, v) in p.into_iter() {
            rv.all_props_mut(k).push(v);
        };
        rv
    }


prop -> (String, PropertyValue)
    = k:prop_name p:(";" p:prop_params {p})? ":" v:prop_value {
        (k, PropertyValue {
            value: v,
            params: match p { Some(x) => x, None => "".to_string() }
        })
    }

prop_name -> String
    = name_char+ { match_str.into_string() }

prop_params -> String
    = prop_char+ { match_str.into_string() }

prop_value -> String
    = value_char+ { match_str.into_string() }

// Characters
name_char = ([a-zA-Z] / "-")
prop_char = name_char / [=;]
value_char = !eol .
eol = "\n" / "\r\n" / "\r" / "\u2028" / "\u2029"

"#)


pub fn parse_item(s: &String) -> Result<Item, String> {
    parser::item(s.as_slice())
}
