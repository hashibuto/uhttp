use std::error;

use crate::{header::HttpHeader, session::TcpSession};

pub struct HttpStatus {
    pub proto: String,
    pub status_msg: String,
    pub status_code: usize,
}

impl HttpStatus {
    pub fn new(status_code: usize) -> Self {
        let status_msg: String;

        // We are a client, this is implemented for testing purposes only
        if status_code < 200 {
            status_msg = "Information".to_owned();
        } else if status_code < 300 {
            status_msg = "OK".to_owned();
        } else if status_code < 400 {
            status_msg = "Redirect".to_owned();
        } else if status_code < 500 {
            status_msg = "Client error".to_owned();
        } else {
            status_msg = "Server error".to_owned();
        }

        Self {
            proto: "HTTP/1.1".to_owned(),
            status_code,
            status_msg,
        }
    }

    pub fn to_string(&self) -> String {
        return format!("{} {} {}", self.proto, self.status_code, self.status_msg);
    }

    pub fn from_string(status: &String) -> Result<Self, Box<dyn error::Error>> {
        let parts: Vec<&str> = status.split(" ").collect();
        if parts.len() != 3 {
            return Err("unable to parse http status header".into());
        }

        let proto = parts[0].to_string();
        let status_code: usize = parts[1].to_string().parse::<usize>()?;
        if status_code < 100 || status_code >= 600 {
            return Err(format!("invalid status code: {}", status_code).into());
        }
        let status_msg = parts[2].to_string();

        return Ok(HttpStatus {
            proto,
            status_code,
            status_msg,
        });
    }
}

pub struct Response {
    pub status: HttpStatus,
    pub session: TcpSession,
    pub header: HttpHeader,

    has_chunked_body: bool,
    next_chunk_bytes_available: usize,
    next_chunk_bytes_read: usize,

    body_bytes_available: usize,
    body_bytes_read: usize,
}

impl Response {
    pub fn from_http_header(
        mut session: TcpSession,
        header: HttpHeader,
    ) -> Result<Self, Box<dyn error::Error>> {
        let status = HttpStatus::from_string(&header.line)?;

        let mut body_bytes_available: usize = 0;
        let content_length_result = header.get_value("content-length".to_owned());
        if content_length_result.is_some() {
            let content_length_str = content_length_result.unwrap();
            body_bytes_available = content_length_str.parse::<usize>()?;
        }

        let mut has_chunked_body = false;
        let transfer_encoding_result = header.get_value("transfer-encoding".to_owned());
        let mut next_chunk_bytes_available: usize = 0;
        if transfer_encoding_result.is_some() {
            // we only support chunked encoding
            let encoding = transfer_encoding_result.unwrap();
            if encoding != "chunked" {
                return Err(
                    format!("transfer encoding of \"{}\" is not supported", encoding).into(),
                );
            }

            has_chunked_body = true;
            next_chunk_bytes_available = session.recv_chunk_header()?;
        }

        Ok(Self {
            body_bytes_available,
            body_bytes_read: 0,
            has_chunked_body,
            next_chunk_bytes_available,
            next_chunk_bytes_read: 0,
            status,
            session,
            header,
        })
    }

    // Returns true if there is a body associated with this response which needs to be read
    pub fn has_body(&self) -> bool {
        self.body_bytes_available > 0 || self.has_chunked_body
    }

    pub fn read_body(&mut self, buf: &mut [u8]) -> Result<usize, Box<dyn error::Error>> {
        if self.has_chunked_body {
            return self._read_body_chunked(buf);
        }

        if self.body_bytes_available > 0 {
            return self._read_body_fixed(buf);
        }

        return Ok(0);
    }

    pub fn read_entire_body(&mut self, max_bytes: usize) -> Result<Vec<u8>, Box<dyn error::Error>> {
        let mut grow_buf: Vec<u8> = vec![];
        let mut buf = [0u8; 4096];
        let mut n_bytes: usize = 1;
        while n_bytes > 0 {
            n_bytes = self.read_body(&mut buf)?;
            if n_bytes > 0 {
                grow_buf.append(&mut grow_buf[..n_bytes].to_vec());
            }
            if grow_buf.len() > max_bytes {
                return Err(format!("body exceeded maximum byte limit of {}", max_bytes).into());
            }
        }

        Ok(grow_buf)
    }

    fn _read_body_fixed(&mut self, buf: &mut [u8]) -> Result<usize, Box<dyn error::Error>> {
        // short circuit out if the body has already been consumed
        if self.body_bytes_available <= self.body_bytes_read {
            return Ok(0);
        }

        let bytes_remaining = self.body_bytes_available - self.body_bytes_read;
        let mut smallest = bytes_remaining;
        if buf.len() < smallest {
            smallest = buf.len();
        }

        let size = self.session.recv(&mut buf[..smallest])?;
        self.body_bytes_read += size;
        return Ok(size);
    }

    fn _read_body_chunked(&mut self, buf: &mut [u8]) -> Result<usize, Box<dyn error::Error>> {
        let mut n_bytes_left = self.next_chunk_bytes_available - self.next_chunk_bytes_read;
        if n_bytes_left == 0 {
            if self.next_chunk_bytes_available == 0 {
                return Ok(0);
            }
            // Receive the final sequence in the chunk, which technically represents the delimiter
            self.session.recv_until(b"\r\n", 2)?;

            let next_chunk_bytes_available = self.session.recv_chunk_header()?;
            self.next_chunk_bytes_available = next_chunk_bytes_available;
            self.next_chunk_bytes_read = 0;
            if self.next_chunk_bytes_available == 0 {
                return Ok(0);
            }
        }

        n_bytes_left = self.next_chunk_bytes_available - self.next_chunk_bytes_read;
        if n_bytes_left == 0 {
            return Ok(0);
        }

        let mut smallest = n_bytes_left;
        if buf.len() < smallest {
            smallest = buf.len();
        }

        let bytes_read = self.session.recv(&mut buf[..smallest])?;
        self.next_chunk_bytes_read += bytes_read;
        return Ok(bytes_read);
    }
}
