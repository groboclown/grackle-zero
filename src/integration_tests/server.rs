//! A simple TCP/IP server, for checking if the client made a connection.

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct TcpServer {
    addr: SocketAddr,
    handle: thread::JoinHandle<()>,
    state: TcpServerState,
}

impl TcpServer {
    pub fn new() -> Result<Self, std::io::Error> {
        let state = TcpServerState::new();
        let server_state = state.clone();
        let listener = TcpListener::bind("localhost:0")?;
        let addr = listener.local_addr()?;
        let handle = thread::spawn(move || {
            for connection in listener.incoming() {
                let shutdown = server_state
                    .access(|s| {
                        s.connected += 1;
                        s.shutdown
                    })
                    .expect("lock poisoned");
                if shutdown {
                    return;
                }
                match connection {
                    Ok(_) => {
                        // Should handle the connection...
                        // But, as this is for testing only whether a connection can be made,
                        // just drop the connection immediately.
                    }
                    Err(e) => {
                        println!("Connection failed: {:?}", e);
                        break;
                    }
                }
            }
        });
        Ok(TcpServer {
            addr,
            handle,
            state,
        })
    }

    /// Shut down the server, and get the number of connections made to it.
    pub fn shutdown(self) -> Result<u64, std::io::Error> {
        self.state.access(|s| {
            s.woke_up += 1;
            s.shutdown = true;
        })?;
        let c = TcpStream::connect(&self.addr)?;
        drop(c);
        self.handle.join().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::AddrNotAvailable, "join failed")
        })?;
        self.state.access(|s| s.connected - s.woke_up)
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

#[derive(Clone)]
struct TcpServerState {
    inner: Arc<Mutex<InnerTcpServerState>>,
}

impl TcpServerState {
    fn new() -> Self {
        TcpServerState {
            inner: Arc::new(Mutex::new(InnerTcpServerState {
                connected: 0,
                woke_up: 0,
                shutdown: false,
            })),
        }
    }

    /// Generic helper to lock the inner state and mutate with a provided closure.
    fn access<R, F>(&self, f: F) -> Result<R, std::io::Error>
    where
        F: FnOnce(&mut InnerTcpServerState) -> R,
    {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "lock poisoned"))?;
        Ok(f(&mut *guard))
    }
}

struct InnerTcpServerState {
    connected: u64,
    woke_up: u64,
    shutdown: bool,
}
