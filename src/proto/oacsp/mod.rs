//
// FlightVars
// Copyright (c) 2015, 2016 Alvaro Polo
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::io;
use std::io::{BufRead, Write};
use std::str::FromStr;

use domain::DomainDispatcher;
use io::*;
use proto::*;
use types::*;

mod input;
mod output;

use self::input::RawInputMessage;
use self::output::RawOutputMessage;

pub struct Oacsp {
    dev: Device,
    domains: DomainDispatcher,
    client_id: Option<String>
}

impl Oacsp {
    pub fn new(dev: Device, domains: DomainDispatcher) -> Oacsp {
        Oacsp { dev: dev, domains: domains, client_id: None }
    }
    
    fn line_is_ready(&self) -> bool {
        self.dev.recv_bytes().contains(&b'\n')
    }

    fn process_input(&mut self) -> io::Result<usize> {
        assert!(self.line_is_ready());
        let dev_id = self.dev.id();
        let mut buf = io::BufReader::new(self.dev.recv_bytes());
        let mut line = String::new();
        let nbytes = try!(buf.read_line(&mut line)) + 1; // end-of-line byte counts
        let begin_received = self.client_id.is_some();
        match (try!(RawInputMessage::from_str(&line)), begin_received) {
            (RawInputMessage::Begin { version: _, client_id }, false) => {
            	self.client_id = Some(client_id);
            	Ok(nbytes)
            },
            (RawInputMessage::Begin { version: _, client_id: _ }, true) => {
				Err(io::Error::new(io::ErrorKind::InvalidData, "begin message already received"))                    
            }
            (RawInputMessage::WriteLvar { lvar, value }, true) => {
                self.domains.with_domain("lvar", |dom| {
					dom.write(&Var::Named(lvar), &value).unwrap(); // TODO: manage errors                        
                });
                Ok(nbytes)
            }
            (RawInputMessage::WriteOffset { offset, value }, true) => {
                self.domains.with_domain("fsuipc", |dom| {
					dom.write(&Var::Offset(offset), &value).unwrap(); // TODO: manage errors                        
                });
                Ok(nbytes)
            }
            (RawInputMessage::ObserveLvar { lvar }, true) => {
                self.domains.with_domain("lvar", |dom| {
					dom.subscribe(dev_id, &Var::Named(lvar)).unwrap(); // TODO: manage errors                        
                });
                Ok(nbytes)
            }
            (RawInputMessage::ObserveOffset { offset }, true) => {
                self.domains.with_domain("fsuipc", |dom| {
					dom.subscribe(dev_id, &Var::Offset(offset)).unwrap(); // TODO: manage errors                        
                });
                Ok(nbytes)
            }
            (_, false) =>  {
                let error = io::Error::new(
                    io::ErrorKind::InvalidData, 
                    "unexpected message while waiting for begin");
				Err(error)                    
            }
        }                
    }
}

impl DeviceHandler for Oacsp {
    fn device(&mut self) -> &mut Device { &mut self.dev }
    
    fn process_event(&mut self, event: Event) {
        match event {
            Event::Ready => {
                self.dev.request_read().unwrap(); // TODO: manage errors
            }
            Event::BytesRead(_) => {
                if self.line_is_ready() {
                    let nread = self.process_input().unwrap(); // TODO: manage errors
                    self.dev.consume_recv_buffer(nread);
                }
                self.dev.request_read().unwrap(); // TODO: manage errors
            }
            Event::BytesWritten(_) => {} // ignored
        }
    }    
}

impl Protocol for Oacsp {
        
    fn send_update(&mut self, domain: &str, variable: Var, value: Value) -> io::Result<()> {
        let raw = try!(match variable {
            Var::Offset(offset) if domain == "fsuipc" =>
                Ok(RawOutputMessage::EventOffset { offset: offset, value: value }),
            Var::Named(ref lvar) if domain == "lvar" =>
                Ok(RawOutputMessage::EventLvar { lvar: lvar.clone(), value: value }),
            _ => {
                Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cannot encode a message for domain '{}', var '{:?}'", 
                        domain, variable)))
            }
        });        
        let mut buf = Vec::new();
        try!(write!(&mut buf, "{}\n", raw));
        self.dev.request_write(&buf)
    }    
}
