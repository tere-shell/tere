use std::any::Any;
use std::boxed::Box;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;

use crate::ipc;

struct State {
    // Can't use ipc::Message as the trait here, as that brings the wrath of object safety on us: <https://doc.rust-lang.org/reference/items/traits.html#object-safety>.
    // If I switch the MAX_SIZE and MAX_FDS consts to functions, 1) they have to have `self` or they trip object safety rules again 2) with `&self` it works, except then we can't downcast to the correct concrete message type anymore.
    // So, stuck with Any we are.
    incoming: VecDeque<Box<dyn Any>>,
    shutdown: bool,
    // Similar scenario here as as with incoming, except this time, we'd be happy with `&Message` instead of `Message`.
    // Can't use `FnOnce(Box<&dyn ipc::Message>)` because of object safety.
    // Unfortunately it seems every checker has to do its own downcasting.
    expectations: VecDeque<Box<dyn FnOnce(&dyn Any)>>,
}

#[derive(Clone)]
pub struct FakeIpc {
    state: Arc<Mutex<State>>,
}

impl FakeIpc {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                incoming: VecDeque::new(),
                shutdown: false,
                expectations: VecDeque::new(),
            })),
        }
    }

    pub fn add<M: 'static + ipc::Message>(&self, message: M) {
        let mut guard = self.state.lock().expect("poisoned mutex");
        guard.incoming.push_back(Box::new(message));
    }

    // After all the incoming messages have been received, make the connection appear shut down.
    pub fn shutdown(&self) {
        let mut guard = self.state.lock().expect("poisoned mutex");
        guard.shutdown = true;
    }

    pub fn expect<F>(&self, f: F)
    where
        F: 'static + FnOnce(&dyn Any),
    {
        let mut guard = self.state.lock().expect("poisoned mutex");
        guard.expectations.push_back(Box::new(f));
    }
}

impl ipc::IPC for FakeIpc {
    fn send_with_fds<M: 'static>(&self, message: &M) -> Result<(), ipc::SendError>
    where
        M: ipc::Message + serde::Serialize,
    {
        println!("send: {:?}", message);
        let exp = {
            let mut guard = self.state.lock().expect("poisoned mutex");
            guard.expectations.pop_front()
        };
        // unlock before calling any callback
        if let Some(exp) = exp {
            let any = message as &dyn Any;
            exp(any);
        }
        Ok(())
    }

    fn receive_with_fds<M>(&self) -> Result<M, ipc::ReceiveError>
    where
        M: 'static + ipc::Message + serde::de::DeserializeOwned,
    {
        let mut guard = self.state.lock().expect("poisoned mutex");
        match guard.incoming.pop_front() {
            Some(b) => {
                let b = b.downcast::<M>().expect("downcast to message M");
                // it's still in a box, so deref
                let m = *b;
                println!("receive: {:?}", m);
                Ok(m)
            }
            None if guard.shutdown => {
                return Err(ipc::ReceiveError::Socket(std::io::Error::from(
                    std::io::ErrorKind::UnexpectedEof,
                )));
            }
            None => panic!("fakeipc has no incoming messages"),
        }
    }
}
