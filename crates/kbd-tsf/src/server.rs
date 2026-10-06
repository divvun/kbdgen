//! The in-process server's lifetime: live COM objects and server locks,
//! which decide `DllCanUnloadNow` (`tsf.component.crate`).

use std::sync::atomic::{AtomicUsize, Ordering};

static OBJECTS: AtomicUsize = AtomicUsize::new(0);
static LOCKS: AtomicUsize = AtomicUsize::new(0);

/// Held by every COM object the server creates; counts it as alive until
/// it is dropped.
#[derive(Debug)]
pub struct Live(());

impl Default for Live {
    fn default() -> Live {
        OBJECTS.fetch_add(1, Ordering::SeqCst);
        Live(())
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

/// `IClassFactory::LockServer`. An unbalanced unlock leaves the count at 0.
pub fn lock(lock: bool) {
    if lock {
        LOCKS.fetch_add(1, Ordering::SeqCst);
    } else {
        let mut locks = LOCKS.load(Ordering::SeqCst);
        while let Some(fewer) = locks.checked_sub(1) {
            match LOCKS.compare_exchange_weak(locks, fewer, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => break,
                Err(actual) => locks = actual,
            }
        }
    }
}

/// Whether no object and no server lock is alive.
// [spec:kbdgen:req:tsf.component.crate]
pub fn can_unload() -> bool {
    OBJECTS.load(Ordering::SeqCst) == 0 && LOCKS.load(Ordering::SeqCst) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:kbdgen:req:tsf.component.crate/test]
    #[test]
    fn unloads_only_without_objects_or_locks() {
        assert!(can_unload());
        let object = Live::default();
        assert!(!can_unload());
        lock(true);
        drop(object);
        assert!(!can_unload());
        lock(false);
        assert!(can_unload());
        lock(false);
        assert!(can_unload());
        lock(true);
        assert!(!can_unload());
        lock(false);
    }
}
