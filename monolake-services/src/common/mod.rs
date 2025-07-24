//! Generic services for panic catching, context management, and timeouts.
pub mod cancel;
pub mod context;
pub mod delay;
pub mod detect;
pub mod erase;
pub mod map;
pub mod panic;
pub mod selector;
pub mod timeout;

// TODO: remove following re-exports
pub use cancel::{linked_list, Canceller, CancellerDropper, Waiter};
pub use context::ContextService;
pub use delay::{Delay, DelayService};
pub use detect::{Detect, DetectService, FixedLengthDetector, PrefixDetector};
pub use erase::EraseResp;
pub use map::{FnSvc, Map, MapErr};
pub use panic::{CatchPanicError, CatchPanicService};
pub use timeout::{Timeout, TimeoutError, TimeoutService};
