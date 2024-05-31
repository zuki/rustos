use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut, Drop};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use aarch64::affinity;

use crate::percore::{self, is_mmu_ready};

#[repr(align(32))]
pub struct Mutex<T> {
    data: UnsafeCell<T>,
    lock: AtomicBool,
    owner: AtomicUsize,
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

pub struct MutexGuard<'a, T: 'a> {
    lock: &'a Mutex<T>,
}

impl<'a, T> !Send for MutexGuard<'a, T> {}
unsafe impl<'a, T: Sync> Sync for MutexGuard<'a, T> {}

impl<T> Mutex<T> {
    pub const fn new(val: T) -> Mutex<T> {
        Mutex {
            lock: AtomicBool::new(false),
            owner: AtomicUsize::new(usize::max_value()),
            data: UnsafeCell::new(val),
        }
    }
}

impl<T> Mutex<T> {
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        if percore::is_mmu_ready() {
            if !self.lock.compare_and_swap(false, true, Ordering::SeqCst) {
                self.owner.store(percore::getcpu(), Ordering::SeqCst);
                Some(MutexGuard { lock: &self })
            } else {
                None
            }
        } else {
            assert!(affinity() == 0);
            if !self.lock.load(Ordering::Relaxed) {
                self.lock.store(true, Ordering::Relaxed);
                self.owner.store(percore::getcpu(), Ordering::Relaxed);
                Some(MutexGuard { lock: &self })
            } else {
                None
            }
        }
    }

    #[inline(never)]
    pub fn lock(&self) -> MutexGuard<T> {
        // lockを「取得」できるまで待機して「取得」する.
        loop {
            match self.try_lock() {
                Some(guard) => return guard,
                None => continue,
            }
        }
    }

    fn unlock(&self) {
        let core = affinity();
        if percore::is_mmu_ready() {
            if self.owner.load(Ordering::Acquire) == core {
                self.owner.store(usize::max_value(), Ordering::Release);
                self.lock.store(false, Ordering::SeqCst);
                percore::putcpu(core);
            }
        } else {
            assert!(core == 0);
            self.owner.store(usize::max_value(), Ordering::Relaxed);
            self.lock.store(false, Ordering::Relaxed);
            percore::putcpu(core);
        }
    }

}

impl<'a, T: 'a> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<'a, T: 'a> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

impl<'a, T: 'a> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.unlock()
    }
}

impl<T: fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => f.debug_struct("Mutex").field("data", &&*guard).finish(),
            None => f.debug_struct("Mutex").field("data", &"<locked>").finish(),
        }
    }
}
