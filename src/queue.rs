use seize::{reclaim, AtomicPtr, Collector, Guard, Linked};
use std::{mem::ManuallyDrop, ptr};
use std::{
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct Queue<T> {
    head: AtomicPtr<Node<T>>,
    tail: AtomicPtr<Node<T>>,
    len: AtomicUsize,
    collector: Collector,
}

#[derive(Debug)]
pub struct Node<T> {
    inner: MaybeUninit<ManuallyDrop<T>>,
    next: AtomicPtr<Node<T>>,
    prev: AtomicPtr<Node<T>>,
}

impl<T> Node<T> {
    fn new(t: T) -> Self {
        Self {
            inner: MaybeUninit::new(ManuallyDrop::new(t)),
            next: AtomicPtr::new(ptr::null_mut()),
            prev: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

impl<T> Queue<T> {
    pub fn new() -> Self {
        let list = Self {
            head: AtomicPtr::new(ptr::null_mut()),
            tail: AtomicPtr::new(ptr::null_mut()),
            collector: Collector::new(),
            len: AtomicUsize::new(0),
        };

        let sentinel = list.collector.link_boxed(Node {
            inner: MaybeUninit::uninit(),
            next: AtomicPtr::new(ptr::null_mut()),
            prev: AtomicPtr::new(ptr::null_mut()),
        });

        list.head.store(sentinel, Ordering::Relaxed);
        list.tail.store(sentinel, Ordering::Relaxed);

        list
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }

    #[inline]
    fn push_back_internal(
        &self,
        onto: *mut Linked<Node<T>>,
        new: *mut Linked<Node<T>>,
        guard: &Guard,
    ) -> bool {
        let next = guard.protect(&unsafe { &*onto }.next, Ordering::Acquire);
        unsafe { &*new }.prev.store(onto, Ordering::Release);

        if !next.is_null() {
            let _ = self
                .tail
                .compare_exchange(onto, next, Ordering::Acquire, Ordering::Relaxed);

            false
        } else {
            let result = unsafe { &*onto }
                .next
                .compare_exchange(ptr::null_mut(), new, Ordering::Release, Ordering::Relaxed)
                .is_ok();

            if result {
                let _ = self
                    .tail
                    .compare_exchange(onto, new, Ordering::Release, Ordering::Relaxed);
            }
            result
        }
    }

    #[inline]
    fn pop_front_internal(&self, guard: &Guard) -> Result<Option<T>, ()> {
        let head = guard.protect(&self.head, Ordering::Acquire);
        let next = guard.protect(&unsafe { &*head }.next, Ordering::Acquire);

        if !next.is_null() {
            match self
                .head
                .compare_exchange(head, next, Ordering::Release, Ordering::Relaxed)
            {
                Ok(_) => {
                    let tail = guard.protect(&self.tail, Ordering::Release);
                    if head == tail {
                        let _ = self.tail.compare_exchange(
                            tail,
                            next,
                            Ordering::Release,
                            Ordering::Relaxed,
                        );
                    }

                    let data = unsafe { ptr::read(&(*next).inner) };
                    Ok(unsafe { self.consume_and_retire(head, data) })
                }
                Err(_) => Err(()),
            }
        } else {
            Ok(None)
        }
    }

    pub fn pop_front(&self) -> Option<T> {
        let guard = self.collector.enter();
        loop {
            if let Ok(head) = self.pop_front_internal(&guard) {
                return head;
            }
        }
    }

    #[inline]
    pub fn push_back(&self, t: T) {
        let guard = self.collector.enter();
        let new = self.collector.link_boxed(Node::new(t));
        loop {
            let tail = guard.protect(&self.tail, Ordering::Acquire);
            if self.push_back_internal(tail, new, &guard) {
                self.len.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    #[inline]
    unsafe fn consume_and_retire(
        &self,
        ptr: *mut Linked<Node<T>>,
        data: MaybeUninit<ManuallyDrop<T>>,
    ) -> Option<T> {
        self.collector.retire(ptr, reclaim::boxed::<Node<T>>);
        self.len.fetch_sub(1, Ordering::Release);
        return Some(ManuallyDrop::into_inner(data.assume_init()));
    }
}

impl<T> Drop for Queue<T> {
    fn drop(&mut self) {
        while self.pop_front().is_some() {}
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Barrier, thread, time::Duration};

    use super::*;

    #[test]
    fn push_back_pop_front() {
        let list = Queue::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        assert_eq!(list.len(), 3);
        assert_eq!(list.pop_front().unwrap(), 1);
        assert_eq!(list.pop_front().unwrap(), 2);
        assert_eq!(list.pop_front().unwrap(), 3);
        assert!(list.pop_front().is_none());
        assert_eq!(list.len(), 0);
    }

    const ITER: u32 = 100;

    #[test]
    fn push_pop_multi() {
        let list = Queue::new();
        let b = Barrier::new(2);
        thread::scope(|s| {
            s.spawn(|| {
                b.wait();
                for i in 0..ITER {
                    list.push_back(i);
                }
            });

            s.spawn(|| {
                b.wait();
                thread::sleep(Duration::from_millis(10));
                for _ in 0..ITER {
                    let _ = list.pop_front();
                }
            });
        });
        assert_eq!(list.len(), 0);
    }
}
