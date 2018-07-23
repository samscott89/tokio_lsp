//! Encoding/decoding of LSP messages
//!
//! A substantial part of this code came from [lsp-client](https://github.com/cmyr/lsp-client/blob/master/src/parsing.rs)

//MIT License

//Copyright (c) 2017 Colin Rothfels
//Copyright (c) 2018 Sam Scott

//Permission is hereby granted, free of charge, to any person obtaining a copy
//of this software and associated documentation files (the "Software"), to deal
//in the Software without restriction, including without limitation the rights
//to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//copies of the Software, and to permit persons to whom the Software is
//furnished to do so, subject to the following conditions:

//The above copyright notice and this permission notice shall be included in all
//copies or substantial portions of the Software.

//THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//OUT OF OR IN CO

use bytes::{BufMut, BytesMut};
use jsonrpc::{BoundaryCodec, Message, Parsed};
use tokio_io::codec::{Decoder, Encoder};

use std::error::Error;
use std::io::{Error as IoError, Result as IoResult, Read};
use std::str;

use super::custom_err;

/// A codec working with LSP messages.
///
/// The implementation is just a simple wrapper around the
/// `tokio_jsonrpc::BoundaryCodec` codec, and just adds/strips the header.
pub struct LspCodec;

impl Encoder for LspCodec {
    type Item = Message;
    type Error = IoError;
    fn encode(&mut self, msg: Message, buf: &mut BytesMut) -> IoResult<()> {
        let mut body = BytesMut::new();
        let mut codec = BoundaryCodec;
        codec.encode(msg, &mut body)?;
        let req_len = body.len();
        buf.reserve(20 + req_len + req_len.to_string().len());
        buf.put(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
        buf.put(body);
        Ok(())
    }
}

impl Decoder for LspCodec {
    type Item = Parsed;
    type Error = IoError;
    fn decode(&mut self, src: &mut BytesMut) -> IoResult<Option<Parsed>> {
        let mut content_length: Option<usize> = None;
        let mut pos = 0;

        if let Some(i) = src.windows(4).position(|b| b == b"\r\n\r\n") {
            let mut header_buf = src.split_to(i + 4);
            let mut buffer = String::new();
            for (idx, _) in header_buf.iter().enumerate().filter(|(_idx, &b)| b == b'\n') {
                buffer.clear();
                (&header_buf[pos..idx]).read_to_string(&mut buffer)?;
                match &buffer {
                    s if s.trim().len() == 0 => { break }, // empty line is end of headers
                    s => {
                        match parse_header(s)? {
                            LspHeader::ContentLength(len) => content_length = Some(len),
                            LspHeader::ContentType => (), // utf-8 only currently allowed value
                        };
                    }
                };
                pos = idx;
            }

            match content_length {
                Some(l) => {
                    if src.len() < l {
                        // Return the header to the buffer
                        header_buf.unsplit(src.take());
                        *src = header_buf;
                        Ok(None)
                    } else {
                        let mut body = src.split_to(l);
                        let mut codec = BoundaryCodec;
                        codec.decode(&mut body)
                    }
                },
                None => {
                    Err(custom_err("Malformed header, missing Content-Length"))
                }
            }
        } else {
            Ok(None)
        }
    }
}


#[derive(Debug, PartialEq)]
/// A message header, as described in the Language Server Protocol specification.
enum LspHeader {
    ContentType,
    ContentLength(usize),
}

const HEADER_CONTENT_LENGTH: &'static [u8] = b"content-length";
const HEADER_CONTENT_TYPE: &'static [u8] = b"content-type";


/// Given a header string, attempts to extract and validate the name and value parts.
fn parse_header(s: &str) -> IoResult<LspHeader> {
    let split: Vec<String> = s.split(": ").map(|s| s.trim().to_lowercase()).collect();
    if split.len() != 2 { return Err(custom_err(&format!("malformed header: {}", s))) }
    match split[0].as_ref() {
        HEADER_CONTENT_TYPE => Ok(LspHeader::ContentType),
        HEADER_CONTENT_LENGTH => Ok(LspHeader::ContentLength(usize::from_str_radix(&split[1], 10).map_err(|e| custom_err(e.description()))?)),
        _ => Err(custom_err(&format!("Unknown header: {}", s))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpc;
    
    #[test]
    fn test_parse_header() {
        let header = "Content-Length: 132";
        assert_eq!(parse_header(header).ok(), Some(LspHeader::ContentLength(132)));
    }

    #[test]
    fn test_parse_message() {
        let msg = jsonrpc::message::from_str("{\"jsonrpc\": \"2.0\",\"id\": 1,\"method\": \"test\"}").unwrap();
        let inps = vec!("Content-Length: 43\r\n\r\n{\"jsonrpc\": \"2.0\",\"id\": 1,\"method\": \"test\"}", 
                        "Content-Length: 43\r\n\r\n{\"jsonrpc\": \"2.0\",\"id\": 1,\"method\": \"test\"}", 
                        "Content-Length: 43\n\rContent-Type: utf-8\r\n\r\n{\"jsonrpc\": \"2.0\",\"id\": 1,\"method\": \"test\"}");

        let mut codec = LspCodec;
        let mut bytes = BytesMut::new();
        for inp in inps {
            // let mut reader = BufReader::new(inp.as_bytes());
            bytes.extend_from_slice(inp.as_bytes());
            let result = match codec.decode(&mut bytes) {
                Ok(r) => r,
                Err(e) => panic!("error: {:?}", e),
            };

            assert_eq!(result.unwrap(), Ok(msg.clone()));
        }
    }

    #[test]
    fn test_partial_message() {
        let msg = jsonrpc::message::from_str("{\"jsonrpc\": \"2.0\",\"id\": 1,\"method\": \"test\"}").unwrap();
        let inps = vec!("Content-Length: 43\r\n\r\n",
                        "{\"jsonrpc\": \"2.0\",\"id\": 1,\"method\": \"test\"}");
        let mut result = None;
        let mut codec = LspCodec;
        let mut bytes = BytesMut::new();
        for inp in inps {
            // let mut reader = BufReader::new(inp.as_bytes());
            bytes.extend_from_slice(inp.as_bytes());
            let res = match codec.decode(&mut bytes) {
                Ok(r) => r,
                Err(e) => panic!("error: {:?}", e),
            };
            if res.is_some() {
                result = res;
            }
        }
        assert_eq!(result.unwrap(), Ok(msg.clone()));
    }
}
