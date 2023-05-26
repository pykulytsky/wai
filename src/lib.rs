use seize::{reclaim, AtomicPtr, Collector};
use std::sync::atomic::Ordering;
use std::{mem::ManuallyDrop, ptr};

pub mod doubly;
pub mod doubly_new;

pub struct LinkedList<T> {
    head: AtomicPtr<Node<T>>,
    collector: Collector,
}

#[derive(Debug)]
pub struct Node<T> {
    inner: ManuallyDrop<T>,
    next: AtomicPtr<Node<T>>,
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        Self {
            head: AtomicPtr::new(ptr::null_mut()),
            collector: Collector::new(),
        }
    }

    #[inline]
    pub fn push_front(&self, t: T) {
        let new = self.collector.link_boxed(Node {
            inner: ManuallyDrop::new(t),
            next: AtomicPtr::new(ptr::null_mut()),
        });

        let guard = self.collector.enter();

        loop {
            let head = guard.protect(&self.head, Ordering::Acquire);
            unsafe { (*new).next.store(head, Ordering::Release) }

            if self
                .head
                .compare_exchange(head, new, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    #[inline]
    pub fn push_back(&self, t: T) {
        let new = self.collector.link_boxed(Node {
            inner: ManuallyDrop::new(t),
            next: AtomicPtr::new(ptr::null_mut()),
        });

        let guard = self.collector.enter();

        let mut current = guard.protect(&self.head, Ordering::Acquire);
        if current.is_null() {
            loop {
                match self
                    .head
                    .compare_exchange(current, new, Ordering::Acquire, Ordering::Relaxed)
                {
                    Ok(_) => {
                        break;
                    }
                    Err(curr) => {
                        current = curr;
                        continue;
                    }
                }
            }
        } else {
            let mut current = guard.protect(&self.head, Ordering::Acquire);
            loop {
                let next = unsafe { &*current }.next.load(Ordering::Acquire);
                if next.is_null() {
                    match unsafe { &*current }.next.compare_exchange(
                        next,
                        new,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => {
                            break;
                        }
                        Err(actual_next) => {
                            // current has next element linked
                            current = actual_next;
                            continue;
                        }
                    }
                } else {
                    // check if next->next is not null
                    let n = unsafe { &*next }.next.load(Ordering::Acquire);
                    if n.is_null() {
                        current = next;
                    } else {
                        current = n;
                    }
                }
            }
        }
    }

    #[inline]
    pub fn pop_front(&self) -> Option<T> {
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
                    let data = ptr::read(&(*head).inner);
                    self.collector.retire(head, reclaim::boxed::<Node<T>>);
                    return Some(ManuallyDrop::into_inner(data));
                }
            }
        }
    }

    pub fn pop_back(&self) -> Option<T> {
        todo!()
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        while self.pop_front().is_some() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_front() {
        let list = LinkedList::new();
        list.push_front(1);
        list.push_front(2);
        list.push_front(3);
        assert_eq!(list.pop_front(), Some(3));
        assert_eq!(list.pop_front(), Some(2));
        assert_eq!(list.pop_front(), Some(1));
    }

    #[test]
    fn push_back() {
        let list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        assert_eq!(list.pop_front(), Some(1));
        assert_eq!(list.pop_front(), Some(2));
        assert_eq!(list.pop_front(), Some(3));
    }
}
