#![allow(dead_code, unused_imports)]

use seize::{reclaim, AtomicPtr, Collector, Guard, Linked};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{mem::ManuallyDrop, ptr};

pub struct LinkedList<T> {
    head: AtomicPtr<Node<T>>,
    tail: AtomicPtr<Node<T>>,
    len: AtomicUsize,
    collector: Collector,
}

#[derive(Debug)]
pub struct Node<T> {
    inner: ManuallyDrop<T>,
    next: AtomicPtr<Node<T>>,
    prev: AtomicPtr<Node<T>>,
}

impl<T> Node<T> {
    fn new(t: T) -> Self {
        Self {
            inner: ManuallyDrop::new(t),
            next: AtomicPtr::new(ptr::null_mut()),
            prev: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
            tail: AtomicPtr::new(ptr::null_mut()),
            collector: Collector::new(),
            len: AtomicUsize::new(0),
        }
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }

    #[inline]
    pub fn push_front(&self, t: T) {
        let guard = self.collector.enter();
        let new = self.collector.link_boxed(Node::new(t));
        let mut head = guard.protect(&self.head, Ordering::Acquire);
        unsafe { &*new }.next.store(head, Ordering::Release);
        if guard.protect(&self.head, Ordering::Acquire).is_null() {
            let mut tail = guard.protect(&self.tail, Ordering::Acquire);
            loop {
                match self
                    .tail
                    .compare_exchange(tail, new, Ordering::AcqRel, Ordering::Relaxed)
                {
                    Ok(_) => break,
                    Err(actual_tail) => {
                        tail = actual_tail;
                        continue;
                    }
                }
            }
        } else {
            // possibly, compare-exchange loop
            unsafe { &*head }.prev.store(new, Ordering::Release);
        }

        loop {
            match self
                .head
                .compare_exchange(head, new, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(actual_head) => {
                    head = actual_head;
                    continue;
                }
            }
        }
        self.len.fetch_add(1, Ordering::Release);
    }

    #[inline]
    pub fn push_back(&self, t: T) {
        let guard = self.collector.enter();
        let new = self.collector.link_boxed(Node::new(t));
        let mut tail = guard.protect(&self.tail, Ordering::Acquire);
        unsafe { &*new }.prev.store(tail, Ordering::Release);
        if guard.protect(&self.tail, Ordering::Acquire).is_null() {
            let mut head = guard.protect(&self.head, Ordering::Acquire);
            loop {
                match self
                    .head
                    .compare_exchange(head, new, Ordering::AcqRel, Ordering::Relaxed)
                {
                    Ok(_) => break,
                    Err(actual_head) => {
                        head = actual_head;
                        continue;
                    }
                }
            }
        } else {
            // possibly, compare-exchange loop
            unsafe { &*tail }.next.store(new, Ordering::Release);
        }

        loop {
            match self
                .tail
                .compare_exchange(tail, new, Ordering::AcqRel, Ordering::Relaxed)
            {
                Ok(_) => break,
                Err(actual_tail) => {
                    tail = actual_tail;
                    continue;
                }
            }
        }

        self.len.fetch_add(1, Ordering::Release);
    }

    #[inline]
    pub fn pop_front(&self) -> Option<T> {
        let guard = self.collector.enter();

        let node = guard.protect(&self.head, Ordering::Acquire);
        if node.is_null() {
            return None;
        }

        let node_next = guard.protect(&unsafe { &*node }.next, Ordering::Acquire);
        self.head.store(node_next, Ordering::Release);

        let head = guard.protect(&self.head, Ordering::Acquire);
        if head.is_null() {
            self.tail.store(ptr::null_mut(), Ordering::Release);
        } else {
            unsafe { &*head }
                .prev
                .store(ptr::null_mut(), Ordering::Release);
        }
        unsafe {
            return self.read_retire(node);
        }
    }

    #[inline]
    pub fn pop_front_old(&self) -> Option<T> {
        let guard = self.collector.enter();

        loop {
            let head = guard.protect(&self.head, Ordering::Acquire);

            if head.is_null() {
                return None;
            }

            let next = unsafe { (*head).next.load(Ordering::Acquire) };

            if self
                .head
                .compare_exchange(head, next, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                unsafe {
                    return self.read_retire(head);
                }
            }
        }
    }

    #[inline]
    pub fn pop_back(&self) -> Option<T> {
        let guard = self.collector.enter();

        let node = guard.protect(&self.tail, Ordering::Acquire);
        if node.is_null() {
            return None;
        }
        let node_prev = guard.protect(&unsafe { &*node }.prev, Ordering::Acquire);
        self.tail.store(node_prev, Ordering::Release);

        let tail = guard.protect(&self.tail, Ordering::Acquire);
        if tail.is_null() {
            self.head.store(ptr::null_mut(), Ordering::Release);
        } else {
            unsafe { &*tail }
                .next
                .store(ptr::null_mut(), Ordering::Release);
        }
        unsafe {
            return self.read_retire(node);
        }
    }

    #[inline]
    pub fn pop_back_old(&self) -> Option<T> {
        let guard = self.collector.enter();

        loop {
            let tail = guard.protect(&self.tail, Ordering::Acquire);

            if tail.is_null() {
                return None;
            }

            let prev = unsafe { (*tail).prev.load(Ordering::Acquire) };

            if self
                .tail
                .compare_exchange(tail, prev, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                unsafe {
                    return self.read_retire(tail);
                }
            }
        }
    }

    #[inline]
    unsafe fn read_retire(&self, ptr: *mut Linked<Node<T>>) -> Option<T> {
        let data = ptr::read(&(*ptr).inner);
        self.collector.retire(ptr, reclaim::boxed::<Node<T>>);
        self.len.fetch_sub(1, Ordering::Release);
        return Some(ManuallyDrop::into_inner(data));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::{sync::Barrier, time::Duration};

    const ITER: u32 = 100;

    #[test]
    fn push_front_1() {
        let list = LinkedList::new();
        list.push_front(1);
        assert_eq!(list.len(), 1);
        assert!(!list.head.load(Ordering::Acquire).is_null());
        assert_eq!(
            list.head.load(Ordering::Acquire),
            list.tail.load(Ordering::Acquire)
        );
    }

    #[test]
    fn push_back_1() {
        let list = LinkedList::new();
        list.push_back(1);
        assert_eq!(list.len(), 1);
        assert!(!list.head.load(Ordering::Acquire).is_null());
        assert_eq!(
            list.head.load(Ordering::Acquire),
            list.tail.load(Ordering::Acquire)
        );
    }

    #[test]
    fn push_front_2() {
        let list = LinkedList::new();
        list.push_front(1);
        list.push_front(1);
        assert_eq!(list.len(), 2);
        assert_ne!(
            list.head.load(Ordering::Acquire),
            list.tail.load(Ordering::Acquire)
        );
    }

    #[test]
    fn push_back_2() {
        let list = LinkedList::new();
        list.push_back(1);
        list.push_back(1);
        assert_eq!(list.len(), 2);
        assert_ne!(
            list.head.load(Ordering::Acquire),
            list.tail.load(Ordering::Acquire)
        );
    }

    #[test]
    fn push_pop_back() {
        let list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        assert_eq!(list.len(), 2);
        assert_eq!(list.pop_back().unwrap(), 2);
        assert_eq!(list.pop_back().unwrap(), 1);
        assert_eq!(list.len(), 0);
        assert!(list.pop_back().is_none());
        assert!(list.head.load(Ordering::Acquire).is_null());
        assert!(list.tail.load(Ordering::Acquire).is_null());
    }

    #[test]
    fn push_pop_front() {
        let list = LinkedList::new();
        list.push_front(1);
        list.push_front(2);
        assert_eq!(list.len(), 2);
        assert_eq!(list.pop_front().unwrap(), 2);
        assert_eq!(list.pop_front().unwrap(), 1);
        assert_eq!(list.len(), 0);
        assert!(list.pop_front().is_none());
        assert!(list.head.load(Ordering::Acquire).is_null());
        assert!(list.tail.load(Ordering::Acquire).is_null());
    }

    #[test]
    fn push_front_pop_back() {
        let list = LinkedList::new();
        list.push_front(1);
        list.push_front(2);
        assert_eq!(list.pop_back().unwrap(), 1);
        assert_eq!(list.pop_back().unwrap(), 2);
        assert!(list.pop_back().is_none());
    }

    #[test]
    fn push_back_pop_front() {
        let list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        assert_eq!(list.pop_front().unwrap(), 1);
        assert_eq!(list.pop_front().unwrap(), 2);
        assert!(list.pop_front().is_none());
    }

    #[test]
    fn push_pop_mult() {
        let list = LinkedList::new();
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
