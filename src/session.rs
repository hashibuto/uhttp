use std::{
    error::{self},
    io::{Read, Write},
    net::TcpStream,
    time::Instant,
    vec,
};

pub struct TcpSession {
    idle_from: Option<Instant>,
    pub host: String,
    is_fresh_conn: bool,
    stream: Option<TcpStream>,
    buffer: Vec<u8>,
}

impl TcpSession {
    pub fn new(host: String) -> Self {
        Self {
            idle_from: None,
            host,
            is_fresh_conn: true,
            stream: None,
            buffer: vec![],
        }
    }

    pub fn from_stream(tcp_stream: TcpStream) -> Self {
        Self {
            idle_from: None,
            host: tcp_stream.peer_addr().unwrap().to_string(),
            is_fresh_conn: true,
            stream: Some(tcp_stream),
            buffer: vec![],
        }
    }

    pub fn set_idle(&mut self) {
        self.idle_from = Some(Instant::now());
        self.is_fresh_conn = false;
    }

    pub fn is_expired(&self, now: &Instant) -> bool {
        if self.idle_from.is_none() {
            return false;
        }

        return now.duration_since(self.idle_from.unwrap()).as_secs() > 15;
    }

    fn connect(&mut self) -> Result<(), std::io::Error> {
        let stream = TcpStream::connect(&self.host)?;
        self.stream = Some(stream);
        self.is_fresh_conn = true;
        self.buffer = vec![];
        Ok(())
    }

    pub fn send(&mut self, buf: &[u8]) -> Result<usize, Box<dyn error::Error>> {
        if self.stream.is_none() {
            let _ = self.connect()?;
        }
        let n_bytes = self._send(buf)?;
        Ok(n_bytes)
    }

    // Receives until a matching sequence of bytes is observed and a buffer up until, and including that sequence is returned, or
    // max bytes has been read, and an error is returned.
    pub fn recv_until(&mut self, seq: &[u8], max: usize) -> Result<Vec<u8>, Box<dyn error::Error>> {
        if self.stream.is_none() {
            let _ = self.connect()?;
        }

        return self._recv_until(seq, max);
    }

    pub fn recv(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        if self.stream.is_none() {
            let _ = self.connect()?;
        }

        if self.buffer.len() > 0 {
            let mut smallest: usize = buf.len();
            if self.buffer.len() < smallest {
                smallest = self.buffer.len();
            }

            for i in 0..smallest {
                buf[i] = self.buffer[i];
            }

            self.buffer = self.buffer[smallest..].to_vec();
            return Ok(smallest);
        }

        let stream = self.stream.as_mut().unwrap();
        return stream.read(buf);
    }

    // returns the number of bytes of the following chunk, excluding the 2 bytes representing the \r\n delimiter
    pub fn recv_chunk_header(&mut self) -> Result<usize, Box<dyn error::Error>> {
        let buf = self.recv_until(b"\r\n", 100)?;
        let chunk_size_str = (String::from_utf8(buf)?).trim().to_owned();
        let chunk_size: usize = chunk_size_str.parse()?;
        return Ok(chunk_size);
    }

    fn _recv_until(&mut self, seq: &[u8], max: usize) -> Result<Vec<u8>, Box<dyn error::Error>> {
        let mut buf: Vec<u8> = vec![];
        let stream = self.stream.as_mut().unwrap();

        let mut total: usize = 0;
        let mut t_buf = [0u8; 4096];
        let seq_len = seq.len();
        let mut start = 0;
        let mut final_index: i32 = -1;
        while total <= max {
            let n_bytes = stream.read(&mut t_buf)?;
            total += n_bytes;
            if n_bytes > 0 {
                buf.append(&mut t_buf[..n_bytes].to_vec());
            }

            let mut o_found = false;
            for i in start..total {
                if i + seq_len > total {
                    break;
                }

                let mut found = true;
                for j in 0..seq_len {
                    if buf[i + j] != seq[j] {
                        found = false;
                        break;
                    }
                }

                if found {
                    o_found = true;
                    final_index = (i + seq_len) as i32;
                }
            }

            if o_found {
                break;
            }

            start = total - seq_len + 1;
            if n_bytes == 0 {
                break;
            }
        }

        if final_index == -1 {
            if total <= max {
                return Err("stream ended before sequence was found".into());
            }
            return Err("unable to find sequence within the supplied maximum bytes".into());
        }

        // buf now contains all the data we wanted, we need to buffer the remainder if any and then return the truncated buffer
        let cutover_index: usize = (final_index + 1) as usize;
        if buf.len() > cutover_index {
            self.buffer = buf[cutover_index..].to_vec();
            buf.truncate(cutover_index);
        }

        Ok(buf)
    }

    fn _send(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        let stream = self.stream.as_mut().unwrap();

        let result = stream.write(buf);
        if result.is_ok() {
            return Ok(result.unwrap());
        }

        if self.is_fresh_conn {
            return Err(result.err().unwrap());
        }

        self.connect()?;
        let stream = self.stream.as_mut().unwrap();
        return stream.write(buf);
    }
}
