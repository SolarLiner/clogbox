use crate::atomic_linked_list::AtomicLinkedList;
use std::sync::Arc;

pub struct Notifier<T> {
    listeners: Arc<AtomicLinkedList<Box<dyn Send + Sync + Fn(&T)>>>,
}

impl<T> Default for Notifier<T> {
    fn default() -> Self {
        Self {
            listeners: Arc::new(AtomicLinkedList::new()),
        }
    }
}

impl<T> Clone for Notifier<T> {
    fn clone(&self) -> Self {
        Self {
            listeners: self.listeners.clone(),
        }
    }
}

impl<T> Notifier<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_listener(&self, listener: impl 'static + Send + Sync + Fn(&T)) {
        self.listeners.push_back(Box::new(listener));
    }

    pub fn notify(&self, value: T) {
        for listener in &*self.listeners {
            listener(&value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::thread;

    #[test]
    fn test_single_thread() {
        let mut notifier = Notifier::<i32>::new();
        let value = Arc::new(Mutex::new(0));
        let value_clone = value.clone();

        notifier.add_listener(move |x| {
            let mut guard = value_clone.lock().unwrap();
            *guard = *x;
        });

        notifier.notify(42);
        assert_eq!(*value.lock().unwrap(), 42);
    }

    #[test]
    fn test_multi_thread_producers() {
        let notifier = Notifier::<i32>::new();
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        notifier.add_listener(move |x| {
            let mut count = counter_clone.lock().unwrap();
            *count += *x;
        });

        let mut handles = vec![];
        for i in 0..10 {
            let notifier = notifier.clone();
            handles.push(thread::spawn(move || {
                notifier.notify(i);
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(*counter.lock().unwrap(), 45); // Sum of 0..10
    }

    #[test]
    fn test_multi_thread_consumers() {
        let mut notifier = Notifier::<i32>::new();
        let results = Arc::new(Mutex::new(Vec::new()));

        for _ in 0..5 {
            let results = results.clone();
            notifier.add_listener(move |x| {
                let mut vec = results.lock().unwrap();
                vec.push(*x);
            });
        }

        notifier.notify(42);

        let final_results = results.lock().unwrap();
        assert_eq!(final_results.len(), 5);
        assert!(final_results.iter().all(|&x| x == 42));
    }
}
