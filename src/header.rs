use std::{
    collections::{hash_map::Entry, HashMap},
    error,
};

use crate::{request::Method, response::HttpStatus, url::Url};

#[derive(Clone)]
pub struct HttpHeader {
    pub line: String,
    pub headers: HashMap<String, Vec<String>>,
}

impl HttpHeader {
    pub fn new() -> Self {
        Self {
            line: String::new(),
            headers: HashMap::new(),
        }
    }

    pub fn set_req_line(&mut self, method: &Method, url: &Url) {
        self.line = format!("{} {} HTTP/1.1", method.as_str(), url.resource());
    }

    pub fn set_status_line(&mut self, http_status: &HttpStatus) {
        self.line = http_status.to_string();
    }

    pub fn add_header(&mut self, key: String, value: String) {
        let k = key.to_lowercase().trim().to_owned();
        let v = value.trim().to_owned();
        match self.headers.entry(k) {
            Entry::Vacant(e) => {
                e.insert(vec![v]);
            }
            Entry::Occupied(mut e) => {
                e.get_mut().push(v);
            }
        }
    }

    pub fn set_header(&mut self, key: String, value: String) {
        let k = key.to_lowercase().trim().to_owned();
        let v = value.trim().to_owned();
        self.headers.insert(k, vec![v]);
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self, Box<dyn error::Error>> {
        let mut http_header = HttpHeader::new();
        let header_string = String::from_utf8(b.to_vec())?;
        let mut first = true;
        let mut found_newline = false;
        for line in header_string.split("\r\n") {
            if first {
                http_header.line = line.to_owned();
                first = false;
                continue;
            }

            if line.len() == 0 {
                found_newline = true;
                continue;
            }

            if found_newline {
                return Err("malformed header, found data after termination marker".into());
            }

            let result = line.split_once(":");
            if result.is_none() {
                return Err(format!("malformed header line: \"{}\"", line).into());
            }

            let (k_str, v_str) = result.unwrap();
            let key = k_str.to_lowercase().trim().to_owned();
            let value = v_str.trim().to_owned();
            let entry = http_header.headers.entry(key);
            match entry {
                Entry::Vacant(e) => {
                    e.insert(vec![value]);
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().push(value);
                }
            }
        }

        Ok(http_header)
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut strings: Vec<String> = vec![];

        strings.push(self.line.clone());
        for (key, values) in self.headers.iter() {
            for value in values {
                strings.push(format!("{}: {}", key, value));
            }
        }

        strings.push("\r\n".to_owned());

        let header_string = strings.join("\r\n");
        return header_string.as_bytes().to_vec();
    }

    // Returns a single value for key if it exists.  It will always be the first
    // header value received for the given key.
    pub fn get_value(&self, key: String) -> Option<String> {
        let v = self.headers.get(&key.to_lowercase());
        if v.is_none() {
            return None;
        }

        return Some(v.unwrap()[0].clone());
    }
}
