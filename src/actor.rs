#[cfg(target_os = "unix")]
use mio::unix::EventedFd;
/// A simple reactor based on mio
use mio::{
    net::{TcpListener, TcpStream, UdpSocket},
    Evented, Registration, Token,
};

use std::collections::*;
use std::io::Result as IOResult;
use std::sync::*;
use std::task::*;
use std::time::*;

/// don't pretend we can support more envent types than mio provides
pub enum Event {
    TcpListener(TcpListener),
    TcpStream(TcpStream),
    UdpSocket(UdpSocket),
    Registration(Registration),
    #[cfg(target_os = "unix")]
    EventedFd(EventedFd),
}

pub struct EventInfo {
    pub ev: Event,
    pub read_waker: Waker,
    pub write_waker: Waker,
}

impl EventInfo {
    pub fn register(&self, poll: &mio::Poll, token: Token) -> IOResult<()> {
        let opts = mio::PollOpt::edge();
        let interest = mio::Ready::readable() | mio::Ready::writable();;
        match &self.ev {
            Event::TcpListener(tl) => tl.register(poll, token, interest, opts),
            Event::TcpStream(ts) => ts.register(poll, token, interest, opts),
            Event::UdpSocket(us) => us.register(poll, token, interest, opts),
            Event::Registration(re) => re.register(poll, token, interest, opts),
            #[cfg(target_os = "unix")]
            Event::EventedFd(fd) => fd.register(poll, token, interest, opts),
        }
    }

    pub fn deregister(&self, poll: &mio::Poll) -> IOResult<()> {
        match &self.ev {
            Event::TcpListener(tl) => tl.deregister(poll),
            Event::TcpStream(ts) => ts.deregister(poll),
            Event::UdpSocket(us) => us.deregister(poll),
            Event::Registration(re) => poll.deregister(re),
            #[cfg(target_os = "unix")]
            Event::EventedFd(fd) => fd.deregister(poll),
        }
    }
}

struct Registry {
    next_token: usize,
    map: HashMap<mio::Token, EventInfo>,
}

impl Registry {
    pub fn new() -> Registry {
        Registry {
            next_token: 0usize,
            map: HashMap::new(),
        }
    }

    pub fn register(&mut self, ev_info: EventInfo, poll: &mio::Poll) -> Token {
        loop {
            let v = self.next_token;
            self.next_token += 1;
            if v == usize::max_value() {
                continue;
            }
            let token = Token(v);
            match self.map.raw_entry_mut().from_key(&token) {
                std::collections::hash_map::RawEntryMut::Occupied(_) => {
                    continue;
                }
                std::collections::hash_map::RawEntryMut::Vacant(v) => {
                    ev_info.register(poll, token);
                    v.insert(token, ev_info);
                    return token;
                }
            }
        }
    }

    pub fn remove(&mut self, token: Token) -> Option<EventInfo> {
        self.map.remove(&token)
    }
}

/// A simple actor based on mio::Poll
pub struct Actor {
    registry: Mutex<Registry>,
    poll: mio::Poll,
}

impl Actor {
    pub fn new() -> IOResult<Actor> {
        let poll = mio::Poll::new()?;
        Ok(Actor {
            registry: Mutex::new(Registry::new()),
            poll,
        })
    }

    pub fn register(&mut self, ev_info: EventInfo) -> IOResult<Token> {
        let token;
        {
            let registry = &mut self.registry.lock().unwrap();
            token = registry.register(ev_info, &self.poll);
        }
        Ok(token)
    }

    pub fn deregister(&mut self, token: Token) -> IOResult<bool> {
        let registry = &mut self.registry.lock().unwrap();
        if let Some(v) = registry.remove(token) {
            v.deregister(&self.poll)?;
            return Ok(true);
        } else {
            return Ok(false);
        }
    }

    pub fn wait_all_events(&mut self, timeout: Option<Duration>) -> IOResult<usize> {
        let mut events = mio::Events::with_capacity(1024);
        self.poll.poll(&mut events, timeout)?;
        let mut ret: usize = 0;
        {
            let mut registry = self.registry.lock().unwrap();
            for ev in events.iter() {
                let token = ev.token();
                match registry.map.get(&token) {
                    None => {
                        continue;
                    }
                    Some(ref mut v) => {
                        let readiness = ev.readiness();
                        if readiness.contains(mio::Ready::readable()) {
                            v.read_waker.wake();
                        }
                        if readiness.contains(mio::Ready::writable()) {
                            v.write_waker.wake();
                        }
                    }
                }
                ret += 1;
            }
        }
        Ok(ret)
    }
}
