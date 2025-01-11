use std::{
    collections::{HashMap, VecDeque},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, LazyLock, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::session::TcpSession;

pub static POOL_INSTANCE: LazyLock<Arc<Mutex<SessionPool>>> = LazyLock::new(|| {
    return Arc::new(Mutex::new(SessionPool::new()));
});

pub struct SessionPool {
    host_lookup: HashMap<String, VecDeque<TcpSession>>,
    kill_chan: Mutex<Option<Sender<bool>>>,
    last_interaction: Instant,
    thread_handle: Option<JoinHandle<()>>,
}

impl Drop for SessionPool {
    fn drop(&mut self) {
        {
            let mut kc = self.kill_chan.lock().unwrap();
            if kc.is_some() {
                let c = kc.as_mut().unwrap();
                c.send(true).unwrap();

                if let Some(handle) = self.thread_handle.take() {
                    handle.join().unwrap();
                }
            }
        }
    }
}

impl SessionPool {
    pub fn new() -> Self {
        Self {
            host_lookup: HashMap::new(),
            kill_chan: Mutex::new(None),
            last_interaction: Instant::now(),
            thread_handle: None,
        }
    }

    pub fn acquire(&mut self, host: &String) -> TcpSession {
        self.last_interaction = Instant::now();
        let sessions_opt = self.host_lookup.get_mut(host);
        if sessions_opt.is_some() {
            let sessions = sessions_opt.unwrap();
            let session = sessions.pop_front().unwrap();
            return session;
        }

        return TcpSession::new(host.clone());
    }

    pub fn release(&mut self, session: TcpSession) {
        self.last_interaction = Instant::now();
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

        // when an item is released to the session pool, we must ensure the cleanup thread is running, which will run while there are items \
        // to be cleaned up (plus a fixed amount of time)
        let mut kc_guard = self.kill_chan.lock().unwrap();
        if kc_guard.is_none() {
            let (tx, rx): (Sender<bool>, Receiver<bool>) = channel();
            *kc_guard = Some(tx);
            let pool_inst = POOL_INSTANCE.clone();
            self.thread_handle = Some(thread::spawn(move || {
                loop {
                    match rx.recv_timeout(Duration::from_secs(5)) {
                        Ok(_) => return,

                        Err(_) => {
                            // timeout received, perform empty check, etc.
                            let mut pi = pool_inst.lock().unwrap();
                            let thread_ending = pi.remove_expired();
                            if thread_ending {
                                let mut guard = pi.kill_chan.lock().unwrap();
                                *guard = None;
                            }
                            return;
                        }
                    }
                }
            }));
        }
    }

    // removes any expired items and returns true if there are no items left
    pub fn remove_expired(&mut self) -> bool {
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

        return self.host_lookup.len() == 0
            && now.duration_since(self.last_interaction).as_secs() > 30;
    }
}
