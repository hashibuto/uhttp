use std::{
    error,
    io::{self},
};

use header::HttpHeader;
use pool::POOL_INSTANCE;
use response::Response;

mod header;
mod pool;
mod request;
mod response;
mod session;
mod url;

pub use request::Request;
pub use request::Method;
pub use url::Url;

const MAX_HEADER_SIZE: usize = 32768;

pub struct HttpClient {
}

impl HttpClient {
    pub fn new() -> Self {
        return Self {
        };
    }

    // Release connection back to the pool after draining any remaining response data
    pub fn release(&self, mut res: Response) -> Result<(), Box<dyn error::Error>> {
        let mut buf = [0u8; 4096];

        // we need to drain the connection of any remaining response body before we can release it to the pool,
        // this should be a no-op if the connection has already been drained and won't attempt to draw further.
        let mut recv_bytes: usize = 1;
        while recv_bytes > 0 {
            recv_bytes = res.read_body(&mut buf)?;
        }

        POOL_INSTANCE.lock().unwrap().release(res.session);

        Ok(())
    }

    pub fn req(&self, req: &Request) -> Result<Response, Box<dyn error::Error>> {
        let empty_body: Vec<u8> = vec![];
        return self._req(req, 0, &mut empty_body.as_slice());
    }

    pub fn req_with_body(
        &self,
        req: &Request,
        body_size: usize,
        body: &mut impl io::BufRead,
    ) -> Result<Response, Box<dyn error::Error>> {
        return self._req(req, body_size, body);
    }

    fn _req(
        &self,
        req: &Request,
        body_size: usize,
        body: &mut impl io::BufRead,
    ) -> Result<Response, Box<dyn error::Error>> {
        // make a copy of the header so that we can apply default headers
        let mut http_header = req.header.clone();
        http_header.set_header("content-length".to_owned(), format!("{}", body_size));
        http_header.set_header("host".to_owned(), req.url.host());
        if body_size > 0 {
            http_header.set_header_if_empty("content-type".to_owned(), "application/octet-stream".to_owned());
        }

        let mut session = POOL_INSTANCE.lock().unwrap().acquire(&req.url.host());
        let header_vec = http_header.to_vec();
        let header_bytes = header_vec.as_slice();
        let mut total: usize = 0;
        while total < header_bytes.len() {
            let n_bytes = session.send(&header_bytes[total..])?;
            total += n_bytes;
        }

        if body_size > 0 {
            let mut send_buf = [0u8; 4096];
            let mut total: usize = 0;
            while total < body_size {
                let mut cur_total: usize = 0;
                let n_bytes = body.read(&mut send_buf)?;
                total += n_bytes;
                while cur_total < n_bytes {
                    cur_total += session.send(&send_buf[cur_total..])?;
                }
            }
        }

        let recv_buf = session.recv_until(b"\r\n\r\n", MAX_HEADER_SIZE)?;
        let resp_header = HttpHeader::from_bytes(&recv_buf)?;
        let response = Response::from_http_header(session, resp_header)?;
        return Ok(response);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::TcpListener,
        thread::{self},
    };

    use request::Method;
    use response::HttpStatus;
    use session::TcpSession;
    use url::Url;

    use super::*;

    #[test]
    fn test_bodyless_req() {
        let listener = TcpListener::bind("localhost:10643").unwrap();

        let jh = thread::spawn(|| {
            let l = listener;
            let (stream, _) = l.accept().unwrap();
            let mut session = TcpSession::from_stream(stream);
            let header_vec = session.recv_until(b"\r\n\r\n", MAX_HEADER_SIZE).unwrap();
            HttpHeader::from_bytes(&header_vec).unwrap();

            let mut resp_header = HttpHeader::new();
            resp_header.set_status_line(&HttpStatus::new(200));
            resp_header.set_header("authorization".to_owned(), "Bearer token".to_owned());
            let resp_header_bytes = resp_header.to_vec();
            session.send(&resp_header_bytes).unwrap();
        });

        let client = HttpClient::new();
        let req = Request::new(
            Method::Post,
            Url::new("http://localhost:10643/path/is/here?abc=1&def=2".to_string()),
        );
        let resp = client.req(&req).unwrap();
        assert!(resp.status.status_code == 200);
        client.release(resp).unwrap();
        jh.join().unwrap();
    }

    #[test]
    fn test_bodyless_req_reuse_conn() {
        let client = HttpClient::new();
        let listener = TcpListener::bind("localhost:10644").unwrap();

        let jh = thread::spawn(|| {
            let l = listener;
            let (stream, _) = l.accept().unwrap();
            let mut session = TcpSession::from_stream(stream);
            for _ in 0..2 {
                let header_vec = session.recv_until(b"\r\n\r\n", MAX_HEADER_SIZE).unwrap();
                HttpHeader::from_bytes(&header_vec).unwrap();

                let mut resp_header = HttpHeader::new();
                resp_header.set_status_line(&HttpStatus::new(200));
                resp_header.set_header("authorization".to_owned(), "Bearer token".to_owned());
                let resp_header_bytes = resp_header.to_vec();
                session.send(&resp_header_bytes).unwrap();
            }
        });

        for _ in 0..2 {
            let req = Request::new(
                Method::Post,
                Url::new("http://localhost:10644/path/is/here?abc=1&def=2".to_string()),
            );
            let resp = client.req(&req).unwrap();
            assert!(resp.status.status_code == 200);
            client.release(resp).unwrap();
        }
        jh.join().unwrap()
    }

    const BODY_SIZE: usize = 96000;
    #[test]
    fn test_send_body() {
        let listener = TcpListener::bind("localhost:10645").unwrap();

        let jh = thread::spawn(|| {
            let l = listener;
            let (stream, _) = l.accept().unwrap();
            let mut session = TcpSession::from_stream(stream);
            let header_vec = session.recv_until(b"\r\n\r\n", MAX_HEADER_SIZE).unwrap();
            let req_header = HttpHeader::from_bytes(&header_vec).unwrap();

            let content_length = req_header.get_value("content-length".to_owned()).unwrap().parse::<usize>().unwrap();
            assert!(content_length == BODY_SIZE);

            let mut recv_body: Vec<u8> = vec![];
            let mut total: usize = 0;
            let mut buf = [0u8; 4096];
            while total < content_length {
                let n_bytes = session.recv(&mut buf).unwrap();
                total += n_bytes;
                recv_body.append(&mut buf[..n_bytes].to_vec());
            }

            let mut v: u8 = 0;
            for i in 0..BODY_SIZE {
                assert!(recv_body[i] == v);
                if v == 255 {
                    v = 0;
                } else {
                    v += 1;
                }
            }

            let mut resp_header = HttpHeader::new();
            resp_header.set_status_line(&HttpStatus::new(200));
            resp_header.set_header("authorization".to_owned(), "Bearer token".to_owned());
            let resp_header_bytes = resp_header.to_vec();
            session.send(&resp_header_bytes).unwrap();
        });

        let client = HttpClient::new();
        let req = Request::new(
            Method::Post,
            Url::new("http://localhost:10645/path/is/here?abc=1&def=2".to_string()),
        );

        let mut body: Vec<u8> = vec![];
        let mut v: u8 = 0;
        for _ in 0..BODY_SIZE {
            body.push(v);
            if v == 255 {
                v = 0;
            } else {
                v += 1;
            }
        }

        let resp = client.req_with_body(&req, body.len(), &mut body.as_slice()).unwrap();
        assert!(resp.status.status_code == 200);
        client.release(resp).unwrap();
        jh.join().unwrap();
    }

    #[test]
    fn test_recv_body() {
        let listener = TcpListener::bind("localhost:10646").unwrap();

        let jh = thread::spawn(|| {
            let l = listener;
            let (stream, _) = l.accept().unwrap();
            let mut session = TcpSession::from_stream(stream);
            let header_vec = session.recv_until(b"\r\n\r\n", MAX_HEADER_SIZE).unwrap();
            HttpHeader::from_bytes(&header_vec).unwrap();

            let mut resp_header = HttpHeader::new();
            resp_header.set_status_line(&HttpStatus::new(200));
            resp_header.set_header("authorization".to_owned(), "Bearer token".to_owned());
            resp_header.set_header("content-length".to_owned(), format!("{}", BODY_SIZE));
            let resp_header_bytes = resp_header.to_vec();
            session.send(&resp_header_bytes).unwrap();

            let mut body: Vec<u8> = vec![];
            let mut v: u8 = 0;
            for _ in 0..BODY_SIZE {
                body.push(v);
                if v == 255 {
                    v = 0;
                } else {
                    v += 1;
                }
            }
            let mut total = 0;
            while total < BODY_SIZE {
                let n_bytes = session.send(&body[total..]).unwrap();
                total += n_bytes;
            }

        });

        let client = HttpClient::new();
        let req = Request::new(
            Method::Post,
            Url::new("http://localhost:10646/path/is/here?abc=1&def=2".to_string()),
        );
        let mut resp = client.req(&req).unwrap();
        assert!(resp.status.status_code == 200);
        assert!(resp.has_body());
        let mut recv_body: Vec<u8> = vec![];
        let mut n_bytes: usize = 1;
        let mut buf = [0u8; 4096];
        while n_bytes > 0 {
            n_bytes = resp.read_body(&mut buf).unwrap();
            if n_bytes > 0 {
                recv_body.append(&mut buf[..n_bytes].to_vec());
            }
        }

        let mut v: u8 = 0;
        for i in 0..BODY_SIZE {
            assert!(recv_body[i] == v);
            if v == 255 {
                v = 0;
            } else {
                v += 1;
            }
        }

        client.release(resp).unwrap();
        jh.join().unwrap();
    }
}
