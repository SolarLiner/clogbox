use std::iter::FromIterator;
use std::marker::PhantomData;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Arc;

struct Node<T> {
    value: Option<T>,
    next: AtomicPtr<Node<T>>,
}

pub struct AtomicLinkedList<T> {
    head: Arc<Node<T>>,
    tail: AtomicPtr<Node<T>>,
    len: AtomicUsize,
    _marker: PhantomData<T>,
}

impl<T> FromIterator<T> for AtomicLinkedList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let list = AtomicLinkedList::new();
        for item in iter {
            list.push_back(item);
        }
        list
    }
}

impl<T> AtomicLinkedList<T> {
    pub fn new() -> Self {
        let sentinel = Arc::new(Node {
            value: None,
            next: AtomicPtr::new(ptr::null_mut()),
        });

        Self {
            head: sentinel.clone(),
            tail: AtomicPtr::new(Arc::as_ptr(&sentinel) as *mut _),
            len: AtomicUsize::new(0),
            _marker: PhantomData,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head.value.is_none()
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    pub fn push_front(&self, value: T) {
        let new_node = Arc::new(Node {
            value: Some(value),
            next: AtomicPtr::new(ptr::null_mut()),
        });

        let new_node_ptr = Arc::into_raw(new_node) as *mut Node<T>;

        loop {
            // Load the current first node after sentinel
            let first = self.head.next.load(Ordering::Acquire);

            // Set our new node's next pointer to the current first node
            unsafe {
                (*new_node_ptr).next.store(first, Ordering::Release);
            }

            // Try to update the sentinel's next pointer to our new node
            if let Ok(_) =
                self.head
                    .next
                    .compare_exchange_weak(first, new_node_ptr, Ordering::AcqRel, Ordering::Acquire)
            {
                // Success - increment length and return
                self.len.fetch_add(1, Ordering::SeqCst);
                break;
            }

            // Failed CAS, another thread modified the list - retry
        }
    }

    pub fn push_back(&self, value: T) {
        let new_node = Arc::new(Node {
            value: Some(value),
            next: AtomicPtr::new(ptr::null_mut()),
        });

        let new_node_ptr = Arc::into_raw(new_node) as *mut Node<T>;

        loop {
            let tail_ptr = self.tail.load(Ordering::Acquire);
            // SAFETY: tail_ptr always comes from a valid Arc we never drop prematurely
            let tail_node = unsafe { &*tail_ptr };
            // Try to CAS tail's next from null to our new node
            if let Ok(_) =
                tail_node
                    .next
                    .compare_exchange_weak(ptr::null_mut(), new_node_ptr, Ordering::AcqRel, Ordering::Acquire)
            {
                // Success, update the tail pointer
                self.tail.store(new_node_ptr, Ordering::Release);
                self.len.fetch_add(1, Ordering::SeqCst);
                break;
            } else {
                // Someone else inserted a node, help push the tail forward
                let next = tail_node.next.load(Ordering::Acquire);
                if !next.is_null() {
                    let _ = self
                        .tail
                        .compare_exchange_weak(tail_ptr, next, Ordering::AcqRel, Ordering::Acquire);
                }
            }
        }
    }

    /// Return an iterator over all values.
    pub fn iter(&self) -> Iter<T> {
        let first_real = self.head.next.load(Ordering::Acquire);
        let last = self.tail.load(Ordering::Acquire);

        Iter {
            front: first_real,
            back: last,
            _marker: PhantomData,
        }
    }
}

pub struct Iter<'a, T> {
    front: *const Node<T>,
    back: *const Node<T>,
    _marker: PhantomData<&'a T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.front.is_null() {
                None
            } else {
                let node = &*self.front;
                let result = node.value.as_ref();

                // Move to the next node
                self.front = node.next.load(Ordering::Acquire);

                result
            }
        }
    }
}

impl<'a, T> IntoIterator for &'a AtomicLinkedList<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        unsafe {
            if self.back.is_null() || self.front.is_null() {
                // Iterator is exhausted
                self.front = ptr::null();
                self.back = ptr::null();
                return None;
            }

            if self.front == self.back {
                // Last element
                let node = &*self.back;
                let result = node.value.as_ref();
                self.front = ptr::null();
                self.back = ptr::null();
                return result;
            }

            // Get the value from the back node
            let node = &*self.back;
            let result = node.value.as_ref();

            // Find the new last node by scanning from the front
            let mut curr = self.front;
            let mut prev = ptr::null();
            while !curr.is_null() && curr != self.back {
                prev = curr;
                curr = (*curr).next.load(Ordering::Acquire);
            }
            self.back = prev;
            result
        }
    }
}

impl<T> Drop for AtomicLinkedList<T> {
    fn drop(&mut self) {
        // Walk from the sentinel and drop all nodes
        let mut curr = self.head.next.load(Ordering::Acquire) as *mut Node<T>;
        while !curr.is_null() {
            unsafe {
                let boxed = Arc::from_raw(curr);
                curr = boxed.next.load(Ordering::Relaxed);
                // Drop happens here
            }
        }
        // head (sentinel) is owned by self.head (Arc), so will be dropped automatically
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_list() {
        let list: AtomicLinkedList<i32> = AtomicLinkedList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_push_front() {
        let list = AtomicLinkedList::new();
        list.push_front(1);
        list.push_front(2);
        assert_eq!(list.len(), 2);
        let items: Vec<_> = list.iter().copied().collect();
        assert_eq!(items, vec![2, 1]);
    }

    #[test]
    fn test_push_back() {
        let list = AtomicLinkedList::new();
        list.push_back(1);
        list.push_back(2);
        assert_eq!(list.len(), 2);
        let items: Vec<_> = list.iter().copied().collect();
        assert_eq!(items, vec![1, 2]);
    }

    #[test]
    fn test_iteration() {
        let list = AtomicLinkedList::from_iter(vec![1, 2, 3]);
        let forward: Vec<_> = list.iter().copied().collect();
        let backward: Vec<_> = list.iter().rev().copied().collect();
        assert_eq!(forward, vec![1, 2, 3]);
        assert_eq!(backward, vec![3, 2, 1]);
    }
}
