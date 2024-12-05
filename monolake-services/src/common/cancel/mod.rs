use std::{
    cell::UnsafeCell,
    future::Future,
    rc::{Rc, Weak},
    task::Waker,
};

use linked_list::LinkedList;

pub mod linked_list;

struct CancelHandler {
    cancelled: bool,
    waiters: LinkedList<Waker>,
}

#[derive(Clone)]
pub struct Canceller {
    handler: Rc<UnsafeCell<CancelHandler>>,
}

impl Default for Canceller {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Canceller {
    pub fn new() -> Self {
        Self {
            handler: Rc::new(UnsafeCell::new(CancelHandler {
                cancelled: false,
                waiters: LinkedList::new(),
            })),
        }
    }

    pub fn waiter(&self) -> Waiter {
        Waiter {
            index: UnsafeCell::new(None),
            handler: Rc::downgrade(&self.handler),
        }
    }

    pub fn cancel(&self) {
        let handler = unsafe { &mut *self.handler.get() };
        if !handler.cancelled {
            handler.cancelled = true;
            let waiters: LinkedList<Waker> =
                std::mem::replace(&mut handler.waiters, LinkedList::new());
            for waker in waiters.into_iter() {
                waker.wake();
            }
        }
    }

    pub const fn dropper(self) -> CancellerDropper {
        CancellerDropper(self)
    }
}

pub struct CancellerDropper(Canceller);

impl Drop for CancellerDropper {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

pub struct Waiter {
    index: UnsafeCell<Option<usize>>,
    handler: Weak<UnsafeCell<CancelHandler>>,
}

impl Clone for Waiter {
    fn clone(&self) -> Self {
        Self {
            index: UnsafeCell::new(None),
            handler: self.handler.clone(),
        }
    }
}

impl Waiter {
    pub fn cancelled(&self) -> bool {
        self.handler
            .upgrade()
            .is_none_or(|handler| unsafe { &*handler.get() }.cancelled)
    }
}

impl Future for Waiter {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let handler = match self.handler.upgrade() {
            Some(handler) => handler,
            None => return std::task::Poll::Ready(()),
        };
        let handler = unsafe { &mut *handler.get() };
        if handler.cancelled {
            return std::task::Poll::Ready(());
        }
        match unsafe { *self.index.get() } {
            Some(idx) => {
                let val = handler.waiters.get_mut(idx).unwrap();
                val.clone_from(cx.waker());
            }
            None => {
                let index = handler.waiters.push_back(cx.waker().clone());
                unsafe { *self.index.get() = Some(index) };
            }
        }
        std::task::Poll::Pending
    }
}

impl Drop for Waiter {
    fn drop(&mut self) {
        if let Some(index) = unsafe { *self.index.get() } {
            if let Some(handler) = self.handler.upgrade() {
                let handler = unsafe { &mut *handler.get() };
                if !handler.cancelled {
                    handler.waiters.remove(index);
                }
            }
        }
    }
}
