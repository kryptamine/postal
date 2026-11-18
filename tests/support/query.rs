//! A URL query or form body, decoded into its fields.

use std::collections::HashMap;

pub type Form = HashMap<String, String>;

pub fn of_url(url: &str) -> Form {
    parse(url.split_once('?').map_or("", |(_, q)| q))
}

pub fn parse(query: &str) -> Form {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(name, value)| (decode(name), decode(value)))
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap();
                out.push(u8::from_str_radix(hex, 16).unwrap());
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }

    String::from_utf8(out).unwrap()
}
