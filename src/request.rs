use crate::{header::HttpHeader, url::Url};

pub enum Method {
    Get,
    Post,
    Put,
    Head,
    Patch,
    Delete,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Patch => "PATCH",
            Method::Put => "PUT",
            Method::Head => "HEAD",
            Method::Delete => "DELETE",
        }
    }
}

pub struct Request {
    pub method: Method,
    pub header: HttpHeader,
    pub url: Url,
}

impl Request {
    pub fn new(method: Method, url: Url) -> Self {
        let mut header = HttpHeader::new();
        header.set_req_line(&method, &url);
        return Request {
            method,
            header,
            url,
        };
    }
}
