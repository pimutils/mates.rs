use std::collections::HashMap;
use std::collections::hashmap::{Occupied, Vacant};


pub struct PropertyValue {
    params: String,
    value: String,
}

impl PropertyValue {
    pub fn get_raw_value(&self) -> &String { &self.value }
    pub fn get_raw_params(&self) -> &String { &self.params }
}


pub struct Item {
    props: HashMap<String, Vec<PropertyValue>>,
}

impl Item {
    pub fn single_value(&self, key: &String) -> Option<&String> {
        match self.props.find(key) {
            Some(x) => { if x.len() > 0 { Some(x[0].get_raw_value()) } else { None } },
            None => { None }
        }
    }

    pub fn all_values(&self, key: &String) -> Option<&Vec<PropertyValue>> {
        self.props.find(key)
    }
}


pub fn parse_item(s: &String) -> Item {
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
    // FIXME: Better way to write this without expect?
    let mut kv_splitresult = s.as_slice().splitn(1, ':');
    let key_and_params = kv_splitresult.next().expect("");
    let value = match kv_splitresult.next() {
        Some(x) => x,
        None => ""
    };

    // FIXME: Better way to write this without expect?
    let mut kp_splitresult = key_and_params.splitn(1, ';');
    let key = kp_splitresult.next().expect("");
    let params = match kp_splitresult.next() {
        Some(x) => x,
        None => ""
    };

    (key.into_string(), PropertyValue {
        value: value.into_string(),
        params: params.into_string()
    })
}

