/// Vec based linked list.
pub struct LinkedList<T> {
    head: usize,
    tail: usize,
    vacancy_head: usize,
    data: Vec<Node<T>>,
}

pub struct Node<T> {
    prev: usize,
    next: usize,
    data: Option<T>,
}

pub const NULL: usize = usize::MAX;

impl<T> Default for LinkedList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LinkedList<T> {
    pub const fn new() -> Self {
        Self {
            head: NULL,
            tail: NULL,
            vacancy_head: NULL,
            data: Vec::new(),
        }
    }

    pub fn get(&self, idx: usize) -> Option<&T> {
        self.data.get(idx).and_then(|node| node.data.as_ref())
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut T> {
        self.data.get_mut(idx).and_then(|node| node.data.as_mut())
    }

    pub fn push_back(&mut self, val: T) -> usize {
        let idx = if self.vacancy_head != NULL {
            let idx = self.vacancy_head;
            let node = &mut self.data[idx];
            self.vacancy_head = node.next;
            node.next = NULL;
            node.data = Some(val);
            idx
        } else {
            let idx = self.data.len();
            self.data.push(Node {
                prev: NULL,
                next: NULL,
                data: Some(val),
            });
            idx
        };

        if self.tail == NULL {
            self.head = idx;
            self.tail = idx;
        } else {
            let tail = &mut self.data[self.tail];
            tail.next = idx;
            self.data[idx].prev = self.tail;
            self.tail = idx;
        }

        idx
    }

    pub fn remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.data.len() {
            return None;
        }

        let node = &mut self.data[idx];
        let val = node.data.take()?;
        let prev = node.prev;
        let next = node.next;

        if prev == NULL {
            self.head = next;
        } else {
            self.data[prev].next = next;
        }

        if next == NULL {
            self.tail = prev;
        } else {
            self.data[next].prev = prev;
        }

        self.data[idx].next = self.vacancy_head;
        self.vacancy_head = idx;
        Some(val)
    }
}

impl<T> Drop for LinkedList<T> {
    // Manually drop the data to make it more efficient.
    fn drop(&mut self) {
        let mut head = self.head;
        while head != NULL {
            let node = &mut self.data[head];
            node.data.take();
            head = node.next;
        }
        unsafe { self.data.set_len(0) };
    }
}

impl<T> IntoIterator for LinkedList<T> {
    type Item = T;
    type IntoIter = LinkedListIter<T>;

    fn into_iter(mut self) -> Self::IntoIter {
        let head = std::mem::replace(&mut self.head, NULL);
        let data = std::mem::take(&mut self.data);
        LinkedListIter { head, data }
    }
}

pub struct LinkedListIter<T> {
    head: usize,
    data: Vec<Node<T>>,
}

impl<T> Iterator for LinkedListIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.head == NULL {
            return None;
        }

        let node = &mut self.data[self.head];
        let val = node.data.take();
        self.head = node.next;
        val
    }
}

impl<T> Drop for LinkedListIter<T> {
    // Manually drop the data to make it more efficient.
    fn drop(&mut self) {
        let mut head = self.head;
        while head != NULL {
            let node = &mut self.data[head];
            node.data.take();
            head = node.next;
        }
        unsafe { self.data.set_len(0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo() {
        let mut linked = LinkedList::new();
        assert_eq!(0, linked.push_back(1));
        assert_eq!(1, linked.push_back(2));
        assert_eq!(2, linked.push_back(3));
        assert_eq!(linked.remove(1).unwrap(), 2);
        assert!(linked.remove(1).is_none());
        assert_eq!(linked.push_back(2333), 1);

        let iter = linked.into_iter();
        assert_eq!(iter.collect::<Vec<_>>(), vec![1, 3, 2333]);
    }
}
