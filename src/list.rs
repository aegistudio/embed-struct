use crate::ptr::NullablePtr;
use std::ops::{Index, IndexMut};

pub struct ListNode<P = usize>
where
    P: NullablePtr,
{
    prev: P,
    next: P,
}

impl<P> ListNode<P>
where
    P: NullablePtr,
{
    pub fn new() -> Self {
        Self {
            prev: P::nullptr(),
            next: P::nullptr(),
        }
    }

    pub fn new_indexed(p: P) -> Self {
        assert!(!p.null());
        Self { prev: p, next: p }
    }

    pub fn in_list(&self) -> bool {
        !self.prev.null()
    }

    pub fn get_prev(&self) -> P {
        self.prev
    }

    pub fn get_next(&self) -> P {
        self.next
    }
}

impl<P> Default for ListNode<P>
where
    P: NullablePtr,
{
    fn default() -> Self {
        ListNode::new()
    }
}

pub trait ListNodeIndex<P = usize>
where
    P: NullablePtr,
{
    type Container;

    fn index(c: &Self::Container, i: P) -> &ListNode<P>;

    fn index_mut(c: &mut Self::Container, i: P) -> &mut ListNode<P>;
}

pub struct ListMut<'a, P, L>
where
    P: NullablePtr,
    L: ListNodeIndex<P>,
{
    list: &'a mut L::Container,
}

impl<'a, P, L> Index<P> for ListMut<'a, P, L>
where
    P: NullablePtr,
    L: ListNodeIndex<P>,
{
    type Output = ListNode<P>;

    fn index(&self, p: P) -> &ListNode<P> {
        L::index(self.list, p)
    }
}

impl<'a, P, L> IndexMut<P> for ListMut<'a, P, L>
where
    P: NullablePtr,
    L: ListNodeIndex<P>,
{
    fn index_mut(&mut self, p: P) -> &mut ListNode<P> {
        L::index_mut(&mut self.list, p)
    }
}

impl<'a, P, L> ListMut<'a, P, L>
where
    P: NullablePtr,
    L: ListNodeIndex<P>,
{
    pub fn new(list: &'a mut L::Container) -> Self {
        Self { list: list }
    }

    /// Remove the p from the list it is in.
    ///
    /// Return the next node of p, or nullptr
    /// if there's no next node of this node.
    ///
    /// The function panics if node p is not
    /// in list. The caller is obligated to
    /// make sure p is in list.
    pub fn remove_from_list(&mut self, p: P) -> P {
        assert!(!p.null());
        assert!(self[p].in_list());
        let prev = self[p].prev;
        let next = self[p].next;
        // WARN: Be wary of the order of setting
        // the pointer. When p is a single node
        // list, given that p.prev = p.next = p,
        // We must update the pointers in p.prev
        // and p.next before setting pointers of
        // p to nullptr.
        self[prev].next = next;
        self[next].prev = prev;
        self[p].prev = P::nullptr();
        self[p].next = P::nullptr();
        if next == p { P::nullptr() } else { next }
    }

    /// Remove p while fixing the head.
    pub fn remove(&mut self, head: &mut P, p: P) {
        let mut dummy = P::nullptr();
        let target = if p == *head { head } else { &mut dummy };
        *target = self.remove_from_list(p);
    }

    /// Pop the last item in the list
    /// and return it.
    ///
    /// If the list is empty, then nullptr
    /// will be returned.
    pub fn pop_back(&mut self, head: &mut P) -> P {
        if head.null() {
            return P::nullptr();
        }
        assert!(self[*head].in_list());
        let last = self[*head].prev;
        self.remove(head, last);
        last
    }

    /// Pop the first item in the list
    /// and return it.
    pub fn pop_front(&mut self, head: &mut P) -> P {
        if head.null() {
            return P::nullptr();
        }
        assert!(self[*head].in_list());
        let first = *head;
        self.remove(head, first);
        first
    }

    /// Clear the list.
    ///
    /// For the list pointed at head, we will
    /// remove every node in the list so that
    /// those node are not in list.
    pub fn clear(&mut self, head: P) {
        self.clear_option(head);
    }

    fn clear_option(&mut self, head: P) -> Option<()> {
        let mut curr = head.to_option()?;
        assert!(self[curr].in_list());
        loop {
            let p = curr.to_option()?;
            self.remove(&mut curr, p);
        }
    }

    // insert p after l after checked.
    //
    // Have checked that p is not in list
    // and l is in list. There must be
    // l -> p -> l.next after insertion.
    fn insert_checked(&mut self, l: P, p: P) {
        let next = self[l].next;
        self[l].next = p;
        self[p].prev = l;
        self[p].next = next;
        self[next].prev = p;
    }

    /// Insert p after l.
    ///
    /// Here, l can be nullptr, p will be
    /// fixed into a one-element list and
    /// returned. Otherwise p will be inserted
    /// after l, and l will be returned.
    ///
    /// The function panics if node p is in
    /// list, since we can't insert p if it
    /// has already been in a list, unless
    /// the caller removes it from a list
    /// first. The function also panics if l
    /// is not nullptr and not in a list.
    pub fn insert_after(&mut self, l: P, p: P) -> P {
        assert!(!p.null());
        assert!(!self[p].in_list());
        if l.null() {
            self[p] = ListNode::new_indexed(p);
            p
        } else {
            assert!(self[l].in_list());
            self.insert_checked(l, p);
            l
        }
    }

    /// Insert p before l.
    ///
    /// The constrain is the same as
    /// insert_after(l, p). See also
    /// the document of that function.
    pub fn insert_before(&mut self, l: P, p: P) -> P {
        assert!(!p.null());
        assert!(!self[p].in_list());
        if l.null() {
            self[p] = ListNode::new_indexed(p);
            p
        } else {
            assert!(self[l].in_list());
            self.insert_checked(self[l].prev, p);
            l
        }
    }

    /// Append to the list specified by head.
    pub fn append(&mut self, head: &mut P, p: P) {
        *head = self.insert_before(*head, p);
    }

    /// Prepend to the list specified by head.
    pub fn prepend(&mut self, head: &mut P, p: P) {
        self.insert_before(*head, p);
        *head = p;
    }

    // Splice p after l after checked.
    //
    // Have checked that both l and p are in
    // list. There must be l -> p -> l.next
    // after splicing.
    fn splice_checked(&mut self, l: P, p: P) {
        let next = self[l].next;
        let prev = self[p].prev;
        self[l].next = p;
        self[p].prev = l;
        self[prev].next = next;
        self[next].prev = prev;
    }

    /// Splice p after l.
    ///
    /// Both p and l must be lists, and the
    /// segment of list which p points to will
    /// be inserted between l and l.next. That is,
    /// l -> p -> l.next then.
    ///
    /// If either l it will be viewed as an empty
    /// list and thus p will be returned. This also
    /// do for p and l will be returned. Otherwise
    /// the caller must guarantee both p and l are
    /// list if they don't want the function to panic.
    pub fn splice_after(&mut self, l: P, p: P) -> P {
        if l.null() {
            return p;
        }
        if p.null() {
            return l;
        }
        assert!(self[p].in_list());
        assert!(self[l].in_list());
        self.splice_checked(l, p);
        l
    }

    /// Splice p before l.
    ///
    /// The calling convention are the same as
    /// splice_after(l, p), except for there will
    /// be l.prev -> p -> l after splicing.
    ///
    /// The result will be p if p is not empty list,
    /// or l will be returned.
    pub fn splice_before(&mut self, l: P, p: P) -> P {
        if l.null() {
            return p;
        }
        if p.null() {
            return l;
        }
        assert!(self[p].in_list());
        assert!(self[l].in_list());
        self.splice_checked(self[l].prev, p);
        p
    }
}

pub struct List<'a, P, L>
where
    P: NullablePtr,
    L: ListNodeIndex<P>,
{
    list: &'a L::Container,
}

impl<'a, P, L> Index<P> for List<'a, P, L>
where
    P: NullablePtr,
    L: ListNodeIndex<P>,
{
    type Output = ListNode<P>;

    fn index(&self, p: P) -> &ListNode<P> {
        L::index(self.list, p)
    }
}

struct ListIterator<'a, 'b, P, L>
where
    P: NullablePtr,
    L: ListNodeIndex<P>,
{
    list: &'b List<'a, P, L>,
    head: P,
    curr: P,
}

impl<'a, 'b, P, L> Iterator for ListIterator<'a, 'b, P, L>
where
    P: NullablePtr,
    L: ListNodeIndex<P>,
{
    type Item = P;
    fn next(&mut self) -> Option<Self::Item> {
        // Empty list has no item to iterate.
        if self.head.null() {
            return None;
        }
        if self.curr == self.head {
            return None;
        }
        let result = if self.curr.null() {
            self.head
        } else {
            self.curr
        };
        self.curr = self.list[result].next;
        Some(result)
    }
}

impl<'a, P, L> List<'a, P, L>
where
    P: NullablePtr,
    L: ListNodeIndex<P>,
{
    pub fn new(list: &'a L::Container) -> Self {
        Self { list: list }
    }

    pub fn iter_head(&self, head: P) -> impl Iterator<Item = P> {
        if !head.null() {
            assert!(self[head].in_list());
        }
        ListIterator {
            list: self,
            head: head,
            curr: P::nullptr(),
        }
    }
}
