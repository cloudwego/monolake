//! Generic services for panic catching, context management, and timeouts.
mod cancel;
mod context;
mod delay;
mod erase;
mod map;
mod panic;
mod timeout;

pub use cancel::{linked_list, Canceller, CancellerDropper, Waiter};
pub use context::ContextService;
pub use delay::{Delay, DelayService};
pub use erase::EraseResp;
pub use map::{FnSvc, Map, MapErr};
pub use panic::{CatchPanicError, CatchPanicService};
pub use timeout::{Timeout, TimeoutError, TimeoutService};
