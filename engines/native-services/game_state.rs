/// A nonblocking state cell for game-owned Rust values. It does not wait on OS
/// synchronization primitives. A concurrent or reentrant borrow returns None.
pub struct GameState<T> {
    occupied: std::sync::atomic::AtomicBool,
    value: std::cell::UnsafeCell<T>,
}

// A successful acquire owns the cell until the unique guard releases it.
unsafe impl<T: Send> Sync for GameState<T> {}

impl<T> GameState<T> {
    pub const fn new(value: T) -> Self {
        Self {
            occupied: std::sync::atomic::AtomicBool::new(false),
            value: std::cell::UnsafeCell::new(value),
        }
    }

    pub fn try_lock(&self) -> Option<GameStateGuard<'_, T>> {
        use std::sync::atomic::Ordering;
        self.occupied
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()?;
        Some(GameStateGuard {
            cell: self,
            borrow: std::marker::PhantomData,
        })
    }
}

pub struct GameStateGuard<'a, T> {
    cell: &'a GameState<T>,
    // The guard has the same Send and Sync bounds as a mutable borrow of T.
    borrow: std::marker::PhantomData<&'a mut T>,
}

impl<T> std::ops::Deref for GameStateGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // This guard holds the only successful borrow.
        unsafe { &*self.cell.value.get() }
    }
}

impl<T> std::ops::DerefMut for GameStateGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // The guard is unique and the caller holds a mutable borrow of it.
        unsafe { &mut *self.cell.value.get() }
    }
}

impl<T> Drop for GameStateGuard<'_, T> {
    fn drop(&mut self) {
        self.cell
            .occupied
            .store(false, std::sync::atomic::Ordering::Release);
    }
}
