use std::{
    collections::{HashMap, VecDeque},
    time::Instant,
};

use crate::session::TcpSession;

pub struct SessionPool {
    host_lookup: HashMap<String, VecDeque<TcpSession>>,
}

impl SessionPool {
    pub fn new() -> Self {
        Self {
            host_lookup: HashMap::new(),
        }
    }

    pub fn acquire(&mut self, host: &String) -> TcpSession {
        let sessions_opt = self.host_lookup.get_mut(host);
        if sessions_opt.is_some() {
            let sessions = sessions_opt.unwrap();
            let session = sessions.pop_front().unwrap();
            return session;
        }

        return TcpSession::new(host.clone());
    }

    pub fn release(&mut self, session: TcpSession) {
        let mut s = session;
        s.set_idle();

        let sessions_opt = self.host_lookup.get_mut(&s.host);
        if sessions_opt.is_some() {
            let sessions = sessions_opt.unwrap();
            sessions.push_back(s);
            return;
        }

        let host = s.host.clone();
        let mut vd = VecDeque::new();
        vd.push_back(s);
        self.host_lookup.insert(host, vd);
    }

    pub fn remove_expired(&mut self) {
        let now = Instant::now();
        let mut rem_hosts: Vec<String> = vec![];
        for (host, sessions) in self.host_lookup.iter_mut() {
            sessions.retain(|x| x.is_expired(&now));
            if sessions.len() == 0 {
                rem_hosts.push(host.clone());
            }
        }

        for host in rem_hosts {
            self.host_lookup.remove(&host);
        }
    }
}
