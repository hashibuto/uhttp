use std::{
    error,
    io::{self},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use header::HttpHeader;
use pool::SessionPool;
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
    kill_chan: Sender<bool>,
    pool: Arc<Mutex<SessionPool>>,
    thread_handle: Option<JoinHandle<()>>,
}

impl Drop for HttpClient {
    fn drop(&mut self) {
        // no choice here but to panic if for whatever reason we cannot invoke the kill channel
        self.kill_chan.send(true).unwrap();
        if let Some(handle) = self.thread_handle.take() {
            handle.join().unwrap();
        }
    }
}

impl HttpClient {
    pub fn new() -> Self {
        let (tx, rx): (Sender<bool>, Receiver<bool>) = channel();
        let pool = Arc::new(Mutex::new(SessionPool::new()));
        let pool_thread_copy = pool.clone();
        let join_handle = thread::spawn(move || {
            loop {
                let result = rx.recv_timeout(Duration::from_secs(5));
                match result {
                    // kill signal received
                    Ok(_) => return,

                    // lock and remove expired entries
                    Err(_) => pool_thread_copy.lock().unwrap().remove_expired(),
                }
            }
        });

        return Self {
            kill_chan: tx,
            pool,
            thread_handle: Some(join_handle),
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

        self.pool.lock().unwrap().release(res.session);

        Ok(())
    }

    pub fn req(&self, req: &Request) -> Result<Response, Box<dyn error::Error>> {
        let empty_body: Vec<u8> = vec![];
        return self._req(req, 0, &mut empty_body.as_slice());
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

        let mut session = self.pool.lock().unwrap().acquire(&req.url.host());
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
    fn test_url_parser() {
        let u = Url::new("http://www.my-test.com:8080/whatever?a=1&b=2#test".to_string());
        assert!(u.scheme == "http");
        assert!(u.hostname == "www.my-test.com");
        assert!(u.port == "8080");
        assert!(u.path == "/whatever");
        assert!(u.query == "a=1&b=2");
        assert!(u.fragment == "test")
    }

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
        jh.join().unwrap()
    }
}
