# uhttp
uhttp (pronounced micro http) is a minimalist http client library focused on providing a narrow subset of HTTP capabilities, enabling simple http comms, without carrying additional dependencies beyond the standard library.

## features
it's not supposed to have a ton of features, it's supposed to be minimalist, hence no compression, etc.
- connection pooling
- chunked encoding

## usage
```
use uhttp::{HttpClient, Method, Request, Url};

fn main() {
  let client = HttpClient::new();
  let req = Request::new(Method::Get, Url::new("http://test.com".to_owned()));
  let mut resp = client.req(&req).unwrap();
  let mut resp_body: Vec<u8> = vec![];
  let mut buf = [0u8; 4096];
  if resp.has_body() {
    let mut bytes_read = 1;
    while bytes_read > 0 {
      bytes_read = resp.read_body(&mut buf).unwrap();
      resp_body.append(&mut buf.to_vec());
    }
  }
  client.release(resp).unwrap();
}
```