//! A mutex-like lock which can be shared between threads and can interact
//! with OpenCL events.
//!

extern crate qutex;

use std::ops::{Deref, DerefMut};
use futures::{/*task,*/ Future, Poll, Async};
use futures::sync::oneshot;
use core::ClContextPtr;
use ::{Event, Result as OclResult, /*Context*/};
use async::{Error as AsyncError, Result as AsyncResult};
// use standard;
pub use self::qutex::qutex::{Request, Guard, FutureGuard, Qutex};

// Allows access to the data contained within a lock just like a mutex guard.
pub struct RwGuard<T> {
    rw_vec: RwVec<T>,
}

impl<T> Deref for RwGuard<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Vec<T> {
        unsafe { &*self.rw_vec.as_ptr() }
    }
}

impl<T> DerefMut for RwGuard<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        unsafe { &mut *self.rw_vec.as_mut_ptr() }
    }
}

impl<T> Drop for RwGuard<T> {
    fn drop(&mut self) {
        unsafe { self.rw_vec.unlock().expect("Error dropping RwGuard") };
    }
}


#[derive(PartialEq)]
enum Stage {
    Marker,
    Qutex,
    Command,
}


/// Like a `FutureGuard` but additionally waits on an OpenCL event.
pub struct PendingRwGuard<T> {
    rw_vec: Option<RwVec<T>>,
    rx: oneshot::Receiver<()>,
    wait_event: Option<Event>,
    trigger_event: Event,
    command_event: Option<Event>,
    stage: Stage,
    len: usize,
}

impl<T> PendingRwGuard<T> {
    fn new<C: ClContextPtr>(rw_vec: RwVec<T>, rx: oneshot::Receiver<()>, context: C,
            wait_event: Option<Event>) -> OclResult<PendingRwGuard<T>>
    {
        let trigger_event = Event::user(context)?;

        let len = unsafe { (*rw_vec.as_ptr()).len() };

        Ok(PendingRwGuard {
            rw_vec: Some(rw_vec),
            rx: rx,
            wait_event: wait_event,
            trigger_event: trigger_event,
            command_event: None,
            stage: Stage::Marker,
            len: len,
        })
    }

    /// Sets the command completion event.
    pub fn set_command_event(&mut self, command_event: Event) {
        self.command_event = Some(command_event);
    }

    /// Returns a reference to the event which will trigger when the wait
    /// marker is complete and the qutex is locked.
    pub fn trigger_event(&self) -> &Event {
        &self.trigger_event
    }

    pub fn wait(self) -> AsyncResult<RwGuard<T>> {
        <Self as Future>::wait(self)
    }

    pub unsafe fn as_mut_ptr(&self) -> Option<*mut T> {
        self.rw_vec.as_ref().map(|rw_vec| (*rw_vec.as_mut_ptr()).as_mut_ptr())
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Polls the wait marker event until all requisite commands have
    /// completed then polls the qutex queue.
    fn poll_marker(&mut self) -> AsyncResult<Async<RwGuard<T>>> {
        debug_assert!(self.stage == Stage::Marker);

        // Check completion of wait event, if it exists:
        if let Some(ref wait_event) = self.wait_event {
            if !wait_event.is_complete()? {
                // let task_ptr = standard::box_raw_void(task::park());
                // unsafe { wait_event.set_callback(standard::_unpark_task, task_ptr)?; };
                wait_event.set_unpark_callback()?;
                return Ok(Async::NotReady);
            }
        }

        self.stage = Stage::Qutex;
        self.poll_qutex()
    }

    /// Polls the qutex until we have obtained a lock then polls the command
    /// event.
    fn poll_qutex(&mut self) -> AsyncResult<Async<RwGuard<T>>> {
        debug_assert!(self.stage == Stage::Qutex);

        // Move the queue along:
        unsafe { self.rw_vec.as_ref().unwrap().process_queue()
            .expect("Error polling PendingRwGuard"); }

        // Check for completion of the rx:
        match self.rx.poll() {
            // If the poll returns `Async::Ready`, we have been popped from
            // the front of the qutex queue and we now have exclusive access.
            // Otherwise, return the `NotReady`. The rx (oneshot channel) will
            // arrange for this task to be awakened when it's ready.
            Ok(status) => {
                match status {
                    Async::Ready(_) => {
                        self.trigger_event.set_complete()?;
                        self.stage = Stage::Command;
                        self.poll_command()
                    },
                    Async::NotReady => Ok(Async::NotReady),
                }
            },
            Err(e) => return Err(e.into()),
        }
    }

    /// Polls the command event until it is complete then returns an `RwGuard`
    /// which can be safely accessed immediately.
    fn poll_command(&mut self) -> AsyncResult<Async<RwGuard<T>>> {
        debug_assert!(self.stage == Stage::Command);

        match self.command_event {
            Some(ref command_event) => {
                if !command_event.is_complete()? {
                    // let task_ptr = standard::box_raw_void(task::park());
                    // unsafe { command_event.set_callback(standard::_unpark_task, task_ptr)?; };
                    command_event.set_unpark_callback()?;
                    return Ok(Async::NotReady);
                } else {
                    Ok(Async::Ready(RwGuard { rw_vec: self.rw_vec.take().unwrap() }))
                }                
            },
            None => Err("PendingRwGuard::poll_command: No command event set.".into()),
        }
    }
}

impl<T> Future for PendingRwGuard<T> {
    type Item = RwGuard<T>;
    type Error = AsyncError;

    #[inline]
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.rw_vec.is_some() {
            match self.stage {
                Stage::Marker => self.poll_marker(),
                Stage::Qutex => self.poll_qutex(),
                Stage::Command => self.poll_command(),
            }            
        } else {
            Err("PendingRwGuard::poll: Task already completed.".into())
        }
    }
}

#[derive(Clone)]
pub struct RwVec<T> {
    qutex: Qutex<Vec<T>>,
}

impl<T> RwVec<T> {
    /// Creates and returns a new `RwVec`.
    #[inline]
    pub fn new() -> RwVec<T> {
        RwVec {
            qutex: Qutex::new(Vec::new())
        }
    }

    pub fn lock_pending_event<C>(&self, context: C, wait_event: Option<Event>) 
            -> OclResult<PendingRwGuard<T>>
            where C: ClContextPtr
    {
        let (tx, rx) = oneshot::channel();
        unsafe { self.qutex.push_request(Request::new(tx)); }
        PendingRwGuard::new((*self).clone().into(), rx, context, wait_event)
    }
}

impl<T> From<Qutex<Vec<T>>> for RwVec<T> {
    fn from(q: Qutex<Vec<T>>) -> RwVec<T> {
        RwVec { qutex: q }
    }
}

impl<T> From<Vec<T>> for RwVec<T> {
    fn from(vec: Vec<T>) -> RwVec<T> {
        RwVec { qutex: Qutex::new(vec) }
    }
}

impl<T> Deref for RwVec<T> {
    type Target = Qutex<Vec<T>>;

    fn deref(&self) -> &Qutex<Vec<T>> {
        &self.qutex
    }
}

impl<T> DerefMut for RwVec<T> {
    fn deref_mut(&mut self) -> &mut Qutex<Vec<T>> {
        &mut self.qutex
    }
}