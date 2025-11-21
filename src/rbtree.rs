use crate::ptr::NullablePtr;
use std::cmp::Ordering;
use std::ops::{Index, IndexMut};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum RBTreeColor {
    RED,
    BLACK,
}

pub mod color {
    use crate::rbtree::RBTreeColor;
    pub const RED: RBTreeColor = RBTreeColor::RED;
    pub const BLACK: RBTreeColor = RBTreeColor::BLACK;
}

/// RBPtrAttr tells the RBTree implementation how to
/// store the index and color.
///
/// This type arises from the idea of storing rbtree
/// attributes into the pointer, since the structs of
/// an RBTree node must be aligned and the lower bits
/// of that points will always be zeroes.
///
/// In our case, we let the user of this module to
/// decide how to handle the storage of the pointer
/// and attribute.
pub trait RBTreePtrColor<P>: Copy + Eq
where
    P: NullablePtr,
{
    fn nullptr_color() -> Self;

    fn combine(p: P, col: RBTreeColor) -> Self;

    fn separate(&self) -> (P, RBTreeColor);

    fn null(&self) -> bool {
        *self == Self::nullptr_color()
    }

    fn ptr(&self) -> P {
        self.separate().0
    }

    fn color(&self) -> RBTreeColor {
        self.separate().1
    }
}

/// For the default behaviour, we borrow one bit from the
/// usize space to store the color of the node. This won't
/// be a problem on 64-bit machines since there's rarely
/// scenario that requires 2**63 - 1 objects.
///
/// Please notice that (usize::MAX >> 1) CANNOT be used as
/// a valid pointer, since
/// nullptr_attr() == combine(usize::MAX >> 1, 1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RBTreePtrColorUsize(usize);

const USIZE_COLOR_MASK: usize = 1 << (usize::BITS - 1);

impl RBTreePtrColor<usize> for RBTreePtrColorUsize {
    fn nullptr_color() -> Self {
        Self(usize::MAX)
    }

    fn combine(p: usize, col: RBTreeColor) -> Self {
        if p.null() {
            Self::nullptr_color()
        } else {
            assert!(p < (usize::MAX >> 1));
            Self(
                p | match col {
                    color::RED => 0,
                    color::BLACK => USIZE_COLOR_MASK,
                },
            )
        }
    }

    fn separate(&self) -> (usize, RBTreeColor) {
        if self.null() {
            (usize::nullptr(), color::BLACK)
        } else {
            let q = self.0;
            (
                (q | USIZE_COLOR_MASK) ^ USIZE_COLOR_MASK,
                match q & USIZE_COLOR_MASK {
                    0 => color::RED,
                    _ => color::BLACK,
                },
            )
        }
    }
}

pub struct RBTreeNode<P = usize, Q = RBTreePtrColorUsize>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
{
    left: P,
    right: P,
    parent_col: Q,
}

impl<P, Q> RBTreeNode<P, Q>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
{
    pub fn new() -> Self {
        Self {
            left: P::nullptr(),
            right: P::nullptr(),
            parent_col: Q::nullptr_color(),
        }
    }

    pub fn in_tree(&self) -> bool {
        !self.parent_col.null()
    }

    pub fn get_color(&self) -> RBTreeColor {
        assert!(self.in_tree());
        self.parent_col.color()
    }

    fn set_color(&mut self, val: RBTreeColor) {
        let (p, _) = Q::separate(&self.parent_col);
        self.parent_col = Q::combine(p, val);
    }

    pub fn get_parent(&self) -> P {
        self.parent_col.ptr()
    }

    fn set_parent(&mut self, p: P) {
        let (_, col) = Q::separate(&self.parent_col);
        self.parent_col = Q::combine(p, col);
    }

    pub fn get_left(&self) -> P {
        self.left
    }

    pub fn get_right(&self) -> P {
        self.right
    }
}

pub trait RBTreeNodeIndex<P = usize, Q = RBTreePtrColorUsize>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
{
    type Container;

    fn index(c: &Self::Container, i: P) -> &RBTreeNode<P, Q>;

    fn index_mut(c: &mut Self::Container, i: P) -> &mut RBTreeNode<P, Q>;
}

pub struct RBTreeMut<'a, P, Q, L>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
    L: RBTreeNodeIndex<P, Q>,
{
    container: &'a mut L::Container,
}

impl<'a, P, Q, L> Index<P> for RBTreeMut<'a, P, Q, L>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
    L: RBTreeNodeIndex<P, Q>,
{
    type Output = RBTreeNode<P, Q>;

    fn index(&self, p: P) -> &RBTreeNode<P, Q> {
        L::index(self.container, p)
    }
}

impl<'a, P, Q, L> IndexMut<P> for RBTreeMut<'a, P, Q, L>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
    L: RBTreeNodeIndex<P, Q>,
{
    fn index_mut(&mut self, p: P) -> &mut Self::Output {
        L::index_mut(self.container, p)
    }
}

impl<'a, P, Q, L> RBTreeMut<'a, P, Q, L>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
    L: RBTreeNodeIndex<P, Q>,
{
    pub fn new(container: &'a mut L::Container) -> Self {
        Self {
            container: container,
        }
    }

    pub fn to_immut(&self) -> RBTree<'_, P, Q, L> {
        RBTree {
            container: self.container,
        }
    }

    fn relink_left_child(&mut self, p: P, l: P) {
        self[p].left = l;
        if !l.null() {
            self[l].set_parent(p);
        }
    }

    fn relink_right_child(&mut self, p: P, r: P) {
        self[p].right = r;
        if !r.null() {
            self[r].set_parent(p);
        }
    }

    fn rotate(&mut self, root: &mut P, n: P) {
        assert!(self[n].in_tree());
        assert!(!self.to_immut().is_root(n));
        let p = self[n].get_parent();
        let is_parent_root = self.to_immut().is_root(p);
        assert!(self[p].left == n || self[p].right == n);
        let (g, x) = self[p].parent_col.separate();
        let (_, y) = self[n].parent_col.separate();
        if self[p].left == n {
            // +---------+      +---------+
            // |     Px  |      |   Ny    |
            // |    / \  |      |  / \    |
            // |   Ny  3 | ---> | 1   Px  |
            // |  / \    |      |    / \  |
            // | 1   2   |      |   2   3 |
            // +---------+      +---------+
            self[p].parent_col = Q::combine(n, x);
            let t2 = self[n].right;
            self.relink_left_child(p, t2);
            self[n].right = p;
        } else {
            // +---------+      +---------+
            // |   Px    |      |     Ny  |
            // |  / \    |      |    / \  |
            // | 1   Ny  | ---> |   Px  3 |
            // |    / \  |      |  / \    |
            // |   2   3 |      | 1   2   |
            // +---------+      +---------+
            self[p].parent_col = Q::combine(n, x);
            let t2 = self[n].left;
            self.relink_right_child(p, t2);
            self[n].left = p;
        }
        if is_parent_root {
            self[n].parent_col = Q::combine(n, y);
            *root = n;
        } else {
            self[n].parent_col = Q::combine(g, y);
            assert!(self[g].left == p || self[g].right == p);
            if self[g].left == p {
                self[g].left = n;
            } else {
                self[g].right = n;
            }
        }
    }

    fn handle_insert_empty_tree(&mut self, root: &mut P, p: P, n: P) -> Option<()> {
        assert!(!n.null());
        self.to_immut().assert_not_in_tree(n);
        if root.null() {
            assert!(p.null());
            self[n].parent_col = Q::combine(n, color::BLACK);
            *root = n;
            // Return None to disrupt the workflow after.
            None
        } else {
            assert!(!p.null());
            Some(())
        }
    }

    fn insert_left_option(&mut self, root: &mut P, p: P, n: P) -> Option<()> {
        self.handle_insert_empty_tree(root, p, n)?;
        assert!(self[p].in_tree());
        assert!(self[p].left.null());
        self[p].left = n;
        self[n].parent_col = Q::combine(p, color::RED);
        self.insert_fixup(root, n);
        Some(())
    }

    pub fn insert_left(&mut self, root: &mut P, p: P, n: P) {
        self.insert_left_option(root, p, n);
    }

    fn insert_right_option(&mut self, root: &mut P, p: P, n: P) -> Option<()> {
        self.handle_insert_empty_tree(root, p, n)?;
        assert!(self[p].in_tree());
        assert!(self[p].right.null());
        self[p].right = n;
        self[n].parent_col = Q::combine(p, color::RED);
        self.insert_fixup(root, n);
        Some(())
    }

    pub fn insert_right(&mut self, root: &mut P, p: P, n: P) {
        self.insert_right_option(root, p, n);
    }

    fn insert_fixup(&mut self, root: &mut P, n: P) {
        let mut n = n;
        assert!(!n.null());
        assert!(self[n].get_color() == color::RED);
        while !self.to_immut().is_root(n) {
            let mut p = self[n].get_parent();
            assert!(p != n);
            if self[p].get_color() == color::BLACK {
                break;
            }
            // Since parent is red and a rbtree cannot
            // have a red root, the grand parent must
            // exist and be black.
            let g = self[p].get_parent();
            assert!(g != p);
            assert!(self[g].get_color() == color::BLACK);
            assert!(self[g].left == p || self[g].right == p);

            if self[g].left == p {
                let u = self[g].right;
                // +-------+      +-------+
                // |   Gb  |      |   Gr* |
                // |  / \  | ---> |  / \  |
                // | Pr Ur |      | Pb Ub |
                // | h  h  |      | h  h  |
                // +-------+      +-------+
                let u_col = self.to_immut().color_nullable(u);
                if u_col == color::RED {
                    assert!(!u.null());
                    assert!(self[u].get_parent() == g);
                    self[g].set_color(color::RED);
                    self[p].set_color(color::BLACK);
                    self[u].set_color(color::BLACK);
                    n = g;
                    continue;
                }
                assert!(u_col == color::BLACK);

                // +---------+      +-----------+
                // |     Gb  |      |       Gb  |
                // |    / \  |      |      / \  |
                // |   Pr Ub |      |     Nr Ub |
                // |  / \ h  | ---> |    / \ h  |
                // | 1b  Nr  |      |   Pr* 3b  |
                // | h  / \  |      |  / \  h   |
                // |   2b 3b |      | 1b 2b     |
                // |   h  h  |      | h   h     |
                // +---------+      +-----------+
                if self[p].right == n {
                    self.rotate(root, n);
                    (p, n) = (n, p);
                }

                // +-----------+      +---------------+      +---------------+
                // |       Gb  |      |       Pr      |      |       Pb      |
                // |      / \  |      |     /   \     |      |     /   \     |
                // |     Pr Ub |      |   Nr     Gb   |      |   Nr     Gr   |
                // |    / \ h  | ---> |  /  \   /  \  | ---> |  /  \   /  \  |
                // |   Nr  3b  |      | 1b  2b 3b  Ub |      | 1b  2b 3b  Ub |
                // |  / \  h   |      | h-1h-1 h   h  |      | h   h  h   h  |
                // | 1b 2b     |      |               |      |               |
                // | h  h      |      |               |      |               |
                // +-----------+      +---------------+      +---------------+
                assert!(self[p].left == n);
                self.rotate(root, p);
                self[p].set_color(color::BLACK);
                self[g].set_color(color::RED);
                return;
            } else {
                let u = self[g].left;
                // +-------+      +-------+
                // |   Gb  |      |   Gr* |
                // |  / \  | ---> |  / \  |
                // | Ur Pr |      | Ub Pb |
                // | h  h  |      | h  h  |
                // +-------+      +-------+
                let u_col = self.to_immut().color_nullable(u);
                if u_col == color::RED {
                    assert!(!u.null());
                    assert!(self[u].get_parent() == g);
                    self[g].set_color(color::RED);
                    self[p].set_color(color::BLACK);
                    self[u].set_color(color::BLACK);
                    n = g;
                    continue;
                }
                assert!(u_col == color::BLACK);

                // +---------+      +-----------+
                // |    Gb   |      |   Gb      |
                // |   / \   |      |  / \      |
                // |  Ub Pr  |      | Ub Nr     |
                // |  h / \  | ---> | h  / \    |
                // |   Nr 3b |      |   1b Pr*  |
                // |  / \ h  |      |   h  / \  |
                // | 1b 2b   |      |     2b 3b |
                // | h  h    |      |     h  h  |
                // +---------+      +-----------+
                if self[p].left == n {
                    self.rotate(root, n);
                    (p, n) = (n, p);
                }

                // +-----------+      +---------------+      +---------------+
                // |   Gb      |      |       Pr      |      |       Pb      |
                // |  / \      |      |     /   \     |      |     /   \     |
                // | Ub Pr     |      |   Gb     Nr   |      |   Gr     Nr   |
                // | h  / \    | ---> |  /  \   /  \  | ---> |  /  \   /  \  |
                // |   1b  Nr  |      | Ub  1b 2b  3b |      | Ub  1b 2b  3b |
                // |   h  / \  |      | h   h  h-1h-1 |      | h   h  h   h  |
                // |     2b 3b |      |               |      |               |
                // |     h  h  |      |               |      |               |
                // +-----------+      +---------------+      +---------------+
                assert!(self[p].right == n);
                self.rotate(root, p);
                self[p].set_color(color::BLACK);
                self[g].set_color(color::RED);
                return;
            }
        }
        if self[*root].get_color() == color::RED {
            self[*root].set_color(color::BLACK);
        }
    }

    // Insert new node j before i.
    pub fn insert_before(&mut self, root: &mut P, i: P, j: P) {
        let l = self[i].left;
        if l.null() {
            self.insert_left(root, i, j);
        } else {
            let k = self.to_immut().rightmost(l);
            self.insert_right(root, k, j);
        }
    }

    // Insert new node j after i.
    pub fn insert_after(&mut self, root: &mut P, i: P, j: P) {
        let r = self[i].right;
        if r.null() {
            self.insert_right(root, i, j);
        } else {
            let k = self.to_immut().leftmost(r);
            self.insert_left(root, k, j);
        }
    }

    pub fn swap(&mut self, a: P, b: P) {
        assert!(!a.null());
        assert!(!b.null());
        if a == b {
            return;
        }
        if !self[a].in_tree() && !self[b].in_tree() {
            return;
        }
        let (ap, x) = self[a].parent_col.separate();
        let al = self[a].left;
        let ar = self[a].right;
        let (bp, y) = self[b].parent_col.separate();
        let bl = self[b].left;
        let br = self[b].right;
        let mut update_a_parent = true;
        let mut update_b_parent = true;

        if a == bp {
            assert!(ap != b);
            assert!(al == b || ar == b);

            if al == b {
                // +----------+      +----------+
                // |     ap   |      |     ap   |
                // |     |    |      |     |    |
                // |   ax=bp  |      |     bx   |
                // |    / \   | ---> |    / \   |
                // |   by  ar |      |   ay  ar |
                // |  / \     |      |  / \     |
                // | bl  br   |      | bl  br   |
                // +----------+      +----------+
                self.relink_left_child(a, bl);
                self.relink_right_child(a, br);
                self[b].left = a;
                self.relink_right_child(b, ar);
            } else {
                // +----------+      +----------+
                // |   ap     |      |   ap     |
                // |   |      |      |   |      |
                // | ax=bp    |      |   bx     |
                // |  / \     | ---> |  / \     |
                // | al  by   |      | al  ay   |
                // |    / \   |      |    / \   |
                // |   bl  br |      |   bl  br |
                // +----------+      +----------+
                self.relink_left_child(a, bl);
                self.relink_right_child(a, br);
                self.relink_left_child(b, al);
                self[b].right = a;
            }
            self[a].parent_col = Q::combine(b, y);
            update_a_parent = false;
        } else if b == ap {
            assert!(a != bp);
            assert!(bl == a || br == a);

            if bl == a {
                // +----------+      +----------+
                // |     bp   |      |     bp   |
                // |     |    |      |     |    |
                // |   by=ap  |      |     ay   |
                // |    / \   | ---> |    / \   |
                // |   ax  br |      |   bx  br |
                // |  / \     |      |  / \     |
                // | al  ar   |      | al  ar   |
                // +----------+      +----------+
                self[a].left = b;
                self.relink_right_child(a, br);
                self.relink_left_child(b, al);
                self.relink_right_child(b, ar);
            } else {
                // +----------+      +----------+
                // |   bp     |      |   bp     |
                // |   |      |      |   |      |
                // | by=ap    |      |   ay     |
                // |  / \     | ---> |  / \     |
                // | bl  ax   |      | bl  bx   |
                // |    / \   |      |    / \   |
                // |   al  ar |      |   al  ar |
                // +----------+      +----------+
                self.relink_left_child(a, bl);
                self[a].right = b;
                self.relink_left_child(b, al);
                self.relink_right_child(b, ar);
            }
            self[b].parent_col = Q::combine(a, x);
            update_b_parent = false;
        } else {
            // +-------------+      +-------------+
            // |   ap    bp  |      |   ap    bp  |
            // |   |     |   |      |   |     |   |
            // |   ax    by  | ---> |   bx    ay  |
            // |  / \   / \  |      |  / \   / \  |
            // | al ar bl br |      | al ar bl br |
            // +-------------+      +-------------+
            self.relink_left_child(a, bl);
            self.relink_right_child(a, br);
            self.relink_left_child(b, al);
            self.relink_right_child(b, ar);
        }
        if update_a_parent {
            if b == bp {
                self[a].parent_col = Q::combine(a, y);
            } else if !bp.null() {
                self[a].parent_col = Q::combine(bp, y);
                assert!(self[bp].left == b || self[bp].right == b);
                if self[bp].left == b {
                    self[bp].left = a;
                } else {
                    self[bp].right = a;
                }
            } else {
                self[a].parent_col = Q::nullptr_color();
            }
        }
        if update_b_parent {
            if a == ap {
                self[b].parent_col = Q::combine(b, x);
            } else if !ap.null() {
                self[b].parent_col = Q::combine(ap, x);
                assert!(self[ap].left == a || self[ap].right == a);
                if self[ap].left == a {
                    self[ap].left = b;
                } else {
                    self[ap].right = b;
                }
            } else {
                self[b].parent_col = Q::nullptr_color();
            }
        }
    }

    pub fn delete(&mut self, root: &mut P, n: P) {
        assert!(!(*root).null());
        assert!(!n.null());
        assert!(self[n].in_tree());

        if !self[n].left.null() && !self[n].right.null() {
            let t = self.to_immut().successor(n);
            assert!(!t.null());
            self.swap(n, t);
            if *root == n {
                *root = t;
            }
        }
        let l = self[n].left;
        let r = self[n].right;
        assert!(l.null() || r.null());
        let c = if !l.null() { l } else { r };

        // Try to push the black color to its only red children.
        if self[n].get_color() == color::BLACK {
            if !c.null() && self[c].get_color() == color::RED {
                if self[c].get_color() == color::RED {
                    self[c].set_color(color::BLACK);
                    self[n].set_color(color::RED);
                }
            }
        }

        // If we can't push the color to its red children, then
        // we are decreasing black height in the tree path from
        // the root to node n, and fixup is inevitable.
        if self[n].get_color() == color::BLACK {
            self.delete_fixup(root, n);
        }

        // Now we are ready to delete the node n.
        let p = self[n].get_parent();
        assert!(!p.null());
        self[n].left = P::nullptr();
        self[n].right = P::nullptr();
        self[n].parent_col = Q::nullptr_color();
        if p == n {
            assert!(*root == n);
            *root = c;
            if !c.null() {
                self[c].parent_col = Q::combine(c, color::BLACK);
            }
        } else {
            assert!(self[p].left == n || self[p].right == n);
            if self[p].left == n {
                self.relink_left_child(p, c);
            } else {
                self.relink_right_child(p, c);
            }
        }
    }

    pub fn delete_fixup(&mut self, root: &mut P, n: P) {
        let mut n = n;
        assert!(!n.null());
        while !self.to_immut().is_root(n) {
            // XXX: instead of coloring the node red when
            // fixing up deletion recursively, we will
            // always fix the red node inside the loop.
            assert!(self[n].get_color() == color::BLACK);
            let p = self[n].get_parent();
            assert!(p != n);
            assert!(self[p].left == n || self[p].right == n);

            // Since n is black and not null, and the
            // tree rooted at p was previously a valid
            // balanced rbtree, its sibling has black height
            // greater than 0, and thus can never be null.
            if self[p].left == n {
                let mut s = self[p].right;
                assert!(!s.null());
                if self[s].get_color() == color::RED {
                    // +----------+      +----------+      +----------+
                    // |   Pb     |      |     Sr   |      |     Sb   |
                    // |  / \     |      |    / \   |      |    / \   |
                    // | Nb* Sr   | ---> |   Pb 2b  | ---> |   Pr 2b  |
                    // | h  / \   |      |  / \ h+1 |      |  / \ h+1 |
                    // |   1b 2b  |      | Nb*1b    |      | Nb*1b    |
                    // |   h+1h+1 |      | h  h+1   |      | h  h+1   |
                    // +----------+      +----------+      +----------+
                    assert!(self[p].get_color() == color::BLACK);
                    self.rotate(root, s);
                    self[p].set_color(color::RED);
                    self[s].set_color(color::BLACK);
                    let t1 = self[p].right;
                    assert!(!t1.null());
                    assert!(self[t1].get_color() == color::BLACK);
                    s = t1;
                }
                assert!(!s.null());
                assert!(self[s].get_color() == color::BLACK);

                // +----------+      +------------+      +------------+
                // |    P     |      |    P       |      |    P       |
                // |   / \    |      |   / \      |      |   / \      |
                // | Nb*  Sb  | ---> | Nb*  2r    | ---> | Nb*  2b    |
                // | h   / \  |      | h   / \    |      | h   / \    |
                // |    2r 4b |      |    1b  Sb  |      |    1b  Sr  |
                // |   / \ h  |      |    h  / \  |      |    h  / \  |
                // |  1b 3b   |      |      3b 4b |      |      3b 4b |
                // |  h  h    |      |      h  h  |      |      h  h  |
                // +----------+      +------------+      +------------+
                {
                    let t2 = self[s].left;
                    let t2c = self.to_immut().color_nullable(t2);
                    let t4 = self[s].right;
                    let t4c = self.to_immut().color_nullable(t4);
                    if t2c == color::RED && t4c == color::BLACK {
                        self.rotate(root, t2);
                        self[t2].set_color(color::BLACK);
                        self[s].set_color(color::RED);
                        s = t2;
                    }
                }

                let t1 = self[s].left;
                let t1c = self.to_immut().color_nullable(t1);
                let t2 = self[s].right;
                let t2c = self.to_immut().color_nullable(t2);

                if t2c == color::RED {
                    // +---------+      +----------+      +----------+      +-----------+
                    // |   Pb    |      |     Sb   |      |     Sb   |      |     Sb    |
                    // |  / \    |      |    / \   |      |    / \   |      |    / \    |
                    // | Nb* Sb  | ---> |   Pb  2r | ---> |   Pb  2r | ---> |   Pb  2b  |
                    // | h  / \  |      |  / \  h  |      |  / \  h  |      |  / \  h+1 |
                    // |   1  2r |      | Nb* 1    |      | Nb* 1    |      | Nb* 1     |
                    // |   h  h  |      | h   h    |      | h   h    |      | h   h     |
                    // +---------+      +----------+      +----------+      +-----------+
                    // +---------+      +----------+      +----------+      +-----------+
                    // |   Pr    |      |     Sb   |      |     Sr   |      |     Sr    |
                    // |  / \    |      |    / \   |      |    / \   |      |    / \    |
                    // | Nb* Sb  | ---> |   Pr  2r | ---> |   Pb  2r | ---> |   Pb  2b  |
                    // | h  / \  |      |  / \  h  |      |  / \  h  |      |  / \  h+1 |
                    // |   1  2r |      | Nb* 1    |      | Nb* 1    |      | Nb* 1     |
                    // |   h  h  |      | h   h    |      | h   h    |      | h   h     |
                    // +---------+      +----------+      +----------+      +-----------+
                    self.rotate(root, s);
                    let pc = self[p].get_color();
                    self[s].set_color(pc);
                    self[p].set_color(color::BLACK);
                    self[t2].set_color(color::BLACK);
                    return;
                }

                assert!(t1c == color::BLACK && t2c == color::BLACK);

                let pc = self[p].get_color();
                if pc == color::RED {
                    // +---------+      +---------+
                    // |   Pr    |      |   Pb    |
                    // |  / \    |      |  / \    |
                    // | Nb* Sb  | ---> | Nb* Sr  |
                    // | h  / \  |      | h  / \  |
                    // |   1b 2b |      |   1b 2b |
                    // |   h  h  |      |   h  h  |
                    // +---------+      +---------+
                    self[p].set_color(color::BLACK);
                    self[s].set_color(color::RED);
                    return;
                } else {
                    // +---------+      +---------+
                    // |   Pb    |      |   Pb*   |
                    // |  / \    |      |  / \    |
                    // | Nb* Sb  | ---> | Nb  Sr  |
                    // | h  / \  |      | h  / \  |
                    // |   1b 2b |      |   1b 2b |
                    // |   h  h  |      |   h  h  |
                    // +---------+      +---------+
                    self[s].set_color(color::RED);
                    n = p;
                }
            } else {
                let mut s = self[p].left;
                assert!(!s.null());
                if self[s].get_color() == color::RED {
                    // +----------+      +-----------+      +-----------+
                    // |     Pb   |      |   Sr      |      |   Sb      |
                    // |    / \   |      |  / \      |      |  / \      |
                    // |   Sr Nb* | ---> | 1b  Pb    | ---> | 1b  Pr    |
                    // |  / \ h   |      | h+1/ \    |      | h+1/ \    |
                    // | 1b 2b    |      |   2b  Nb* |      |   2b  Nb* |
                    // | h+1h+1   |      |   h+1 h   |      |   h+1 h   |
                    // +----------+      +-----------+      +-----------+
                    assert!(self[p].get_color() == color::BLACK);
                    self.rotate(root, s);
                    self[p].set_color(color::RED);
                    self[s].set_color(color::BLACK);
                    let t2 = self[p].left;
                    assert!(!t2.null());
                    assert!(self[t2].get_color() == color::BLACK);
                    s = t2;
                }
                assert!(!s.null());
                assert!(self[s].get_color() == color::BLACK);

                // +----------+      +------------+      +------------+
                // |     P    |      |       P    |      |       P    |
                // |    / \   |      |      / \   |      |      / \   |
                // |   Sb Nb* |      |     3r Nb* |      |     3b Nb* |
                // |  / \ h   | ---> |    / \ h   | ---> |    / \ h   |
                // | 1b  3r   |      |   Sb  4b   |      |   Sr  4b   |
                // | h  / \   |      |  / \  h    |      |  / \  h    |
                // |   2b 4b  |      | 1b 2b      |      | 1b 2b      |
                // |   h  h   |      | h  h       |      | h  h       |
                // +----------+      +------------+      +------------+
                {
                    let t1 = self[s].left;
                    let t1c = self.to_immut().color_nullable(t1);
                    let t3 = self[s].right;
                    let t3c = self.to_immut().color_nullable(t3);
                    if t1c == color::BLACK && t3c == color::RED {
                        self.rotate(root, t3);
                        self[t3].set_color(color::BLACK);
                        self[s].set_color(color::RED);
                        s = t3;
                    }
                }

                let t1 = self[s].left;
                let t1c = self.to_immut().color_nullable(t1);
                let t2 = self[s].right;
                let t2c = self.to_immut().color_nullable(t2);

                if t1c == color::RED {
                    // +----------+      +----------+      +----------+      +----------+
                    // |     Pb   |      |   Sb     |      |   Sb     |      |   Sb     |
                    // |    / \   |      |  / \     |      |  / \     |      |  / \     |
                    // |   Sb Nr* | ---> | 1r  Pb   | ---> | 1r  Pb   | ---> | 1b  Pb   |
                    // |  / \ h   |      | h  / \   |      | h  / \   |      | h+1/ \   |
                    // | 1r  2    |      |   2  Nr* |      |   2  Nr* |      |   2  Nr* |
                    // | h   h    |      |   h  h   |      |   h  h   |      |   h  h   |
                    // +----------+      +----------+      +----------+      +----------+
                    // +----------+      +----------+      +----------+      +----------+
                    // |     Pr   |      |   Sb     |      |   Sr     |      |   Sr     |
                    // |    / \   |      |  / \     |      |  / \     |      |  / \     |
                    // |   Sb Nr* | ---> | 1r  Pr   | ---> | 1r  Pb   | ---> | 1b  Pb   |
                    // |  / \ h   |      | h  / \   |      | h  / \   |      | h+1/ \   |
                    // | 1r  2    |      |   2  Nr* |      |   2  Nr* |      |   2  Nr* |
                    // | h   h    |      |   h  h   |      |   h  h   |      |   h  h   |
                    // +----------+      +----------+      +----------+      +----------+
                    self.rotate(root, s);
                    let pc = self[p].get_color();
                    self[s].set_color(pc);
                    self[p].set_color(color::BLACK);
                    self[t1].set_color(color::BLACK);
                    return;
                }

                assert!(t1c == color::BLACK && t2c == color::BLACK);

                let pc = self[p].get_color();
                if pc == color::RED {
                    // +----------+      +----------+
                    // |     Pr   |      |     Pb   |
                    // |    / \   |      |    / \   |
                    // |   Sb Nb* | ---> |   Sr Nb* |
                    // |  / \ h   |      |  / \ h   |
                    // | 1b 2b    |      | 1b 2b    |
                    // | h  h     |      | h  h     |
                    // +----------+      +----------+
                    self[p].set_color(color::BLACK);
                    self[s].set_color(color::RED);
                    return;
                } else {
                    // +----------+      +----------+
                    // |     Pb   |      |     Pb*  |
                    // |    / \   |      |    / \   |
                    // |   Sb Nb* | ---> |   Sr  Nb |
                    // |  / \ h   |      |  / \  h  |
                    // | 1b 2b    |      | 1b 2b    |
                    // | h  h     |      | h  h     |
                    // +----------+      +----------+
                    self[s].set_color(color::RED);
                    n = p;
                }
            }
        }
    }
}

pub struct RBTree<'a, P, Q, L>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
    L: RBTreeNodeIndex<P, Q>,
{
    container: &'a L::Container,
}

impl<'a, P, Q, L> Index<P> for RBTree<'a, P, Q, L>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
    L: RBTreeNodeIndex<P, Q>,
{
    type Output = RBTreeNode<P, Q>;

    fn index(&self, p: P) -> &RBTreeNode<P, Q> {
        L::index(self.container, p)
    }
}

impl<'a, P, Q, L> RBTree<'a, P, Q, L>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
    L: RBTreeNodeIndex<P, Q>,
{
    pub fn new(container: &'a L::Container) -> Self {
        Self {
            container: container,
        }
    }

    fn assert_not_in_tree(&self, p: P) {
        assert!(self[p].parent_col.null());
        assert!(self[p].left.null());
        assert!(self[p].right.null());
    }

    fn is_root(&self, p: P) -> bool {
        assert!(!p.null());
        assert!(self[p].in_tree());
        self[p].get_parent() == p
    }

    fn color_nullable(&self, p: P) -> RBTreeColor {
        if p.null() {
            color::BLACK
        } else {
            self[p].get_color()
        }
    }

    // Returns the leftmost node of the
    // subtree rooted at p.
    pub fn leftmost(&self, p: P) -> P {
        assert!(!p.null());
        let mut result = p;
        while !self[result].left.null() {
            result = self[result].left;
        }
        result
    }

    // Returns the rightmost node of the
    // subtree rooted at p.
    pub fn rightmost(&self, p: P) -> P {
        assert!(!p.null());
        let mut result = p;
        while !self[result].right.null() {
            result = self[result].right;
        }
        result
    }

    // Returns the immediate successor of P
    // in the tree, or null if no successor.
    pub fn successor(&self, p: P) -> P {
        assert!(!p.null());
        if !self[p].right.null() {
            return self.leftmost(self[p].right);
        }
        let mut n = p;
        loop {
            if self.is_root(n) {
                return P::nullptr();
            }
            let q = self[n].get_parent();
            if self[q].left == n {
                return q;
            }
            assert!(self[q].right == n);
            n = q;
        }
    }

    // Returns the immediate predecessor of P
    // in the tree, or null if no predecessor.
    pub fn predecessor(&self, p: P) -> P {
        assert!(!p.null());
        if !self[p].left.null() {
            return self.rightmost(self[p].left);
        }
        let mut n = p;
        loop {
            if self.is_root(n) {
                return P::nullptr();
            }
            let q = self[n].get_parent();
            if self[q].right == n {
                return q;
            }
            assert!(self[q].left == n);
            n = q;
        }
    }
}

pub trait RBTreeNodeIndexKey<P = usize, Q = RBTreePtrColorUsize>: RBTreeNodeIndex<P, Q>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
{
    type Key: std::cmp::Ord;

    fn key(container: &Self::Container, i: P) -> &Self::Key;
}

impl<'a, P, Q, L> RBTree<'a, P, Q, L>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
    L: RBTreeNodeIndexKey<P, Q>,
{
    fn key(&self, i: P) -> &'a L::Key {
        L::key(self.container, i)
    }

    // Locate the position in the rbtree with specified root
    // so that the node with key k can be placed.
    //
    // The result j will not be empty iff the tree is not empty.
    // If k < key[j], then self[j].left must be empty.
    // If k > key[j], then self[j].right must be empty.
    pub fn locate(&self, root: P, k: &L::Key) -> P {
        if root.null() {
            return P::nullptr();
        }
        let mut j = root;
        loop {
            assert!(!j.null());
            match k.cmp(self.key(j)) {
                Ordering::Equal => return j,
                Ordering::Less => {
                    let l = self[j].left;
                    if l.null() {
                        return j;
                    }
                    j = l;
                }
                Ordering::Greater => {
                    let r = self[j].right;
                    if r.null() {
                        return j;
                    }
                    j = r;
                }
            }
        }
    }

    // Find any node j such that k == key[j],
    // or null if no such node can be found.
    pub fn find(&self, root: P, k: &L::Key) -> P {
        let j = self.locate(root, k);
        if j.null() {
            return P::nullptr();
        }
        if k.cmp(self.key(j)) != Ordering::Equal {
            return P::nullptr();
        }
        j
    }

    // With i == self.locate(k), find the
    // first node j such that key[i] == k.
    fn shift_first(&self, i: P) -> P {
        assert!(!i.null());
        let k = self.key(i);
        let mut i = i;
        loop {
            let l = self[i].left;
            if l.null() {
                break;
            }
            let j = self.locate(l, k);
            if k.cmp(self.key(j)) != Ordering::Equal {
                break;
            }
            i = j;
        }
        i
    }

    // Find first node j such that k == key[j],
    // or null if no such node can be found.
    pub fn find_first(&self, root: P, k: &L::Key) -> P {
        let j = self.find(root, k);
        if j.null() {
            return j;
        }
        self.shift_first(j)
    }

    // With i == self.locate(k), find the
    // first node j such that key[j] == k.
    fn shift_last(&self, i: P) -> P {
        assert!(!i.null());
        let k = self.key(i);
        let mut i = i;
        loop {
            let r = self[i].right;
            if r.null() {
                break;
            }
            let j = self.locate(r, k);
            if k.cmp(self.key(j)) != Ordering::Equal {
                break;
            }
            i = j;
        }
        i
    }

    // Find last node j such that k == key[j].
    // or null if no such node can be found.
    pub fn find_last(&self, root: P, k: &L::Key) -> P {
        let j = self.find(root, k);
        if j.null() {
            return j;
        }
        self.shift_last(j)
    }
}

impl<'a, P, Q, L> RBTreeMut<'a, P, Q, L>
where
    P: NullablePtr,
    Q: RBTreePtrColor<P>,
    L: RBTreeNodeIndexKey<P, Q>,
{
    // Find the node j such that key[j] == key[i],
    // or else insert i into the rbtree.
    pub fn find_or_insert(&mut self, root: &mut P, i: P) -> P {
        let k = self.to_immut().key(i);
        let j = self.to_immut().locate(*root, k);
        if j.null() {
            self.insert_left(root, *root, i);
            return P::nullptr();
        }
        match k.cmp(L::key(self.container, j)) {
            Ordering::Equal => j,
            Ordering::Less => {
                self.insert_left(root, j, i);
                P::nullptr()
            }
            Ordering::Greater => {
                self.insert_right(root, j, i);
                P::nullptr()
            }
        }
    }

    // Insert the node into the rbtree. Duplicate
    // nodes with the same key will be expected,
    // and the insert location is arbitrary.
    pub fn insert(&mut self, root: &mut P, i: P) {
        self.insert_option(root, i);
    }

    fn insert_option(&mut self, root: &mut P, i: P) -> Option<()> {
        let j = self.find_or_insert(root, i).to_option()?;
        self.insert_after(root, j, i);
        Some(())
    }

    // Insert to node into the rbtree. Duplicate
    // nodes with the same key will be expected,
    // and the newly insert node will appear first
    // of them all.
    pub fn insert_first(&mut self, root: &mut P, i: P) {
        self.insert_first_option(root, i);
    }

    fn insert_first_option(&mut self, root: &mut P, i: P) -> Option<()> {
        let j = self.find_or_insert(root, i).to_option()?;
        let j = self.to_immut().shift_first(j);
        self.insert_before(root, j, i);
        Some(())
    }

    // Insert to node into the rbtree. Duplicate
    // nodes with the same key will be expected,
    // and the newly insert node will appear last
    // of them all.
    pub fn insert_last(&mut self, root: &mut P, i: P) {
        self.insert_last_option(root, i);
    }

    fn insert_last_option(&mut self, root: &mut P, i: P) -> Option<()> {
        let j = self.find_or_insert(root, i).to_option()?;
        let j = self.to_immut().shift_last(j);
        self.insert_after(root, j, i);
        Some(())
    }
}

#[cfg(test)]
mod test {
    use crate::rbtree::*;

    struct RBTreeNodeIndexVec {}

    impl RBTreeNodeIndex for RBTreeNodeIndexVec {
        type Container = Vec<RBTreeNode>;

        fn index(c: &Self::Container, i: usize) -> &RBTreeNode {
            &c[i]
        }

        fn index_mut(c: &mut Self::Container, i: usize) -> &mut RBTreeNode {
            &mut c[i]
        }
    }

    macro_rules! vec_rbtree {
        ($p: expr) => {
            RBTree::<_, _, RBTreeNodeIndexVec>::new(&($p))
        };
    }

    macro_rules! vec_rbtree_mut {
        ($p: expr) => {
            RBTreeMut::<_, _, RBTreeNodeIndexVec>::new(&mut ($p))
        };
    }

    fn rbtree_node_make(n: &mut RBTreeNode, l: usize, r: usize, p: usize, c: RBTreeColor) {
        n.left = l;
        n.right = r;
        n.parent_col = RBTreePtrColorUsize::combine(p, c);
    }

    fn rbtree_node_assert(n: &RBTreeNode, l: usize, r: usize, p: usize, c: RBTreeColor) {
        assert_eq!(n.left, l);
        assert_eq!(n.right, r);
        assert_eq!(n.parent_col, RBTreePtrColorUsize::combine(p, c),);
    }

    #[test]
    fn test_rotate() {
        let mut vec: Vec<RBTreeNode> = Vec::new();
        vec.resize_with(8, RBTreeNode::new);

        #[allow(non_snake_case)]
        let N = usize::nullptr(); // For convenience.

        // +---------------+
        // |      4b       |
        // |     /   \     |
        // |   2r     6r   |
        // |  /  \   /  \  |
        // | 1b  3b 5b  7b |
        // +---------------+
        rbtree_node_make(&mut vec[1], N, N, 2, color::BLACK);
        rbtree_node_make(&mut vec[3], N, N, 2, color::BLACK);
        rbtree_node_make(&mut vec[2], 1, 3, 4, color::RED);
        rbtree_node_make(&mut vec[5], N, N, 6, color::BLACK);
        rbtree_node_make(&mut vec[7], N, N, 6, color::BLACK);
        rbtree_node_make(&mut vec[6], 5, 7, 4, color::RED);
        rbtree_node_make(&mut vec[4], 2, 6, 4, color::BLACK);
        let mut root = 4;

        // +---------------+      +---------------+
        // |      4b       |      |       4b      |
        // |     /   \     |      |      /  \     |
        // |   2r     6r   | ---> |     3b*  6r   |
        // |  /  \   /  \  |      |    /    /  \  |
        // | 1b 3b* 5b  7b |      |   2r   5b  7b |
        // +---------------+      |  /            |
        //                        | 1b            |
        //                        +---------------+
        vec_rbtree_mut!(vec).rotate(&mut root, 3);
        assert_eq!(root, 4);
        rbtree_node_assert(&vec[4], 3, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[3], 2, N, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 1, N, 3, color::RED);
        rbtree_node_assert(&vec[1], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 7, 4, color::RED);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[7], N, N, 6, color::BLACK);

        // +---------------+      +--------------+
        // |       4b      |      |         6r*  |
        // |      /  \     |      |        /  \  |
        // |     3b   6r*  | ---> |       4b  7b |
        // |    /    /  \  |      |      /  \    |
        // |   2r   5b  7b |      |     3b  5b   |
        // |  /            |      |    /         |
        // | 1b            |      |   2r         |
        // +---------------+      |  /           |
        //                        | 1b           |
        //                        +--------------+
        vec_rbtree_mut!(vec).rotate(&mut root, 6);
        assert_eq!(root, 6);
        rbtree_node_assert(&vec[6], 4, 7, 6, color::RED);
        rbtree_node_assert(&vec[4], 3, 5, 6, color::BLACK);
        rbtree_node_assert(&vec[3], 2, N, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 1, N, 3, color::RED);
        rbtree_node_assert(&vec[1], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 4, color::BLACK);
        rbtree_node_assert(&vec[7], N, N, 6, color::BLACK);

        // +--------------+      +-------------+
        // |         6r   |      |       6r    |
        // |        /  \  |      |      /  \   |
        // |       4b  7b |      |     3b*  7b |
        // |      /  \    |      |    /  \     |
        // |     3b*  5b  | ===> |   2r   4b   |
        // |    /         |      |  /      \   |
        // |   2r         |      | 1b       5b |
        // |  /           |      +-------------+
        // | 1b           |
        // +--------------+
        vec_rbtree_mut!(vec).rotate(&mut root, 3);
        assert_eq!(root, 6);
        rbtree_node_assert(&vec[6], 3, 7, 6, color::RED);
        rbtree_node_assert(&vec[3], 2, 4, 6, color::BLACK);
        rbtree_node_assert(&vec[2], 1, N, 3, color::RED);
        rbtree_node_assert(&vec[1], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[4], N, 5, 3, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 4, color::BLACK);
        rbtree_node_assert(&vec[7], N, N, 6, color::BLACK);

        // +-------------+       +-------------+
        // |       6r    |       |     3b*     |
        // |      /  \   |       |    /  \     |
        // |     3b*  7b |       |   2r   6r   |
        // |    /  \     | --->  |  /    /  \  |
        // |   2r   4b   |       | 1b   4b  7b |
        // |  /      \   |       |       \     |
        // | 1b       5b |       |        5b   |
        // +-------------+       +-------------+
        vec_rbtree_mut!(vec).rotate(&mut root, 3);
        assert_eq!(root, 3);
        rbtree_node_assert(&vec[3], 2, 6, 3, color::BLACK);
        rbtree_node_assert(&vec[2], 1, N, 3, color::RED);
        rbtree_node_assert(&vec[1], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[6], 4, 7, 3, color::RED);
        rbtree_node_assert(&vec[4], N, 5, 6, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 4, color::BLACK);
        rbtree_node_assert(&vec[7], N, N, 6, color::BLACK);
    }

    #[test]
    fn test_insert() {
        let mut vec: Vec<RBTreeNode> = Vec::new();
        vec.resize_with(11, RBTreeNode::new);
        let mut root = usize::nullptr();

        #[allow(non_snake_case)]
        let N = usize::nullptr(); // For convenience.

        // 3b*
        let empty = root;
        vec_rbtree_mut!(vec).insert_left(&mut root, empty, 3);
        rbtree_node_assert(&vec[3], N, N, 3, color::BLACK);

        // +--------+
        // | 3b     |
        // |   \    |
        // |    5r* |
        // +--------+
        vec_rbtree_mut!(vec).insert_right(&mut root, 3, 5);
        rbtree_node_assert(&vec[3], N, 5, 3, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 3, color::RED);

        // +-------+      +----------+      +--------+
        // | 3b    |      | 3b       |      |   4b   |
        // |   \   |      |   \      |      |  /  \  |
        // |    5r | ---> |    4r    | ---> | 3r  5r |
        // |   /   |      |     \    |      +--------+
        // | 4r*   |      |      5r* |
        // +-------+      +----------+
        vec_rbtree_mut!(vec).insert_left(&mut root, 5, 4);
        assert_eq!(root, 4);
        rbtree_node_assert(&vec[4], 3, 5, 4, color::BLACK);
        rbtree_node_assert(&vec[3], N, N, 4, color::RED);
        rbtree_node_assert(&vec[5], N, N, 4, color::RED);

        // +-----------+      +----------+      +----------+
        // |   4b      |      |   4r*    |      |   4b     |
        // |  /  \     |      |  /  \    |      |  /  \    |
        // | 3r  5r    | ---> | 3b  5b   | ---> | 3b  5b   |
        // |       \   |      |       \  |      |       \  |
        // |       8r* |      |       8r |      |       8r |
        // +-----------+      +----------+      +----------+
        vec_rbtree_mut!(vec).insert_right(&mut root, 5, 8);
        assert_eq!(root, 4);
        rbtree_node_assert(&vec[4], 3, 5, 4, color::BLACK);
        rbtree_node_assert(&vec[3], N, N, 4, color::BLACK);
        rbtree_node_assert(&vec[5], N, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[8], N, N, 5, color::RED);

        // +------------+
        // |     4b     |
        // |    /  \    |
        // |   3b  5b   |
        // |  /      \  |
        // | 0r*     8r |
        // +------------+
        vec_rbtree_mut!(vec).insert_left(&mut root, 3, 0);
        assert_eq!(root, 4);
        rbtree_node_assert(&vec[4], 3, 5, 4, color::BLACK);
        rbtree_node_assert(&vec[3], 0, N, 4, color::BLACK);
        rbtree_node_assert(&vec[0], N, N, 3, color::RED);
        rbtree_node_assert(&vec[5], N, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[8], N, N, 5, color::RED);

        // +------------+      +--------------+      +------------+
        // |     4b     |      |       4b     |      |     4b     |
        // |    /  \    |      |      /  \    |      |    /  \    |
        // |   3b  5b   |      |     3b  5b   |      |   2b  5b   |
        // |  /      \  | ---> |    /      \  | ---> |  /  \   \  |
        // | 0r      8r |      |   2r      8r |      | 0r  3r  8r |
        // |  \         |      |  /           |      +------------+
        // |   2r*      |      | 0r*          |
        // +------------+      +--------------+
        vec_rbtree_mut!(vec).insert_right(&mut root, 0, 2);
        assert_eq!(root, 4);
        rbtree_node_assert(&vec[4], 2, 5, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::BLACK);
        rbtree_node_assert(&vec[0], N, N, 2, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::RED);
        rbtree_node_assert(&vec[5], N, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[8], N, N, 5, color::RED);

        // +------------+      +------------+
        // |     4b     |      |     4b     |
        // |    /  \    |      |    /  \    |
        // |   2b  5b   |      |   2r  5b   |
        // |  /  \   \  | ---> |  /  \   \  |
        // | 0r  3r  8r |      | 0b  3b  8r |
        // |  \         |      |  \         |
        // |   1r*      |      |   1r       |
        // +------------+      +------------+
        vec_rbtree_mut!(vec).insert_right(&mut root, 0, 1);
        assert_eq!(root, 4);
        rbtree_node_assert(&vec[4], 2, 5, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::RED);
        rbtree_node_assert(&vec[0], N, 1, 2, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[5], N, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[8], N, N, 5, color::RED);

        // +------------+      +---------------+      +---------------+
        // |     4b     |      |     4b        |      |       4b      |
        // |    /  \    |      |    /  \       |      |    /     \    |
        // |   2r  5b   |      |   2r  5b      |      |   2r     6b   |
        // |  /  \   \  | ---> |  /  \   \     | ---> |  /  \   /  \  |
        // | 0b  3b  8r |      | 0b  3b  6r    |      | 0b  3b 5r  8r |
        // |  \     /   |      |  \        \   |      |  \            |
        // |   1r  6r*  |      |   1r      8r* |      |   1r          |
        // +------------+      +---------------+      +---------------+
        vec_rbtree_mut!(vec).insert_left(&mut root, 8, 6);
        assert_eq!(root, 4);
        rbtree_node_assert(&vec[4], 2, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::RED);
        rbtree_node_assert(&vec[0], N, 1, 2, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 6, color::RED);
        rbtree_node_assert(&vec[8], N, N, 6, color::RED);

        // +---------------+      +---------------+
        // |       4b      |      |       4b      |
        // |    /     \    |      |    /     \    |
        // |   2r     6b   |      |   2r     6r   |
        // |  /  \   /  \  | ---> |  /  \   /  \  |
        // | 0b  3b 5r  8r |      | 0b  3b 5b  8b |
        // |  \         /  |      |  \         /  |
        // |   1r     7r*  |      |   1r     7r   |
        // +---------------+      +---------------+
        vec_rbtree_mut!(vec).insert_left(&mut root, 8, 7);
        assert_eq!(root, 4);
        rbtree_node_assert(&vec[4], 2, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::RED);
        rbtree_node_assert(&vec[0], N, 1, 2, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 8, 4, color::RED);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 6, color::BLACK);
        rbtree_node_assert(&vec[7], N, N, 8, color::RED);

        // +-------------------+
        // |       4b          |
        // |    /     \        |
        // |   2r     6r       |
        // |  /  \   /  \      |
        // | 0b  3b 5b  8b     |
        // |  \         / \    |
        // |   1r     7r  10r* |
        // +-------------------+
        vec_rbtree_mut!(vec).insert_right(&mut root, 8, 10);
        assert_eq!(root, 4);
        rbtree_node_assert(&vec[4], 2, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::RED);
        rbtree_node_assert(&vec[0], N, 1, 2, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 8, 4, color::RED);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[8], 7, 10, 6, color::BLACK);
        rbtree_node_assert(&vec[7], N, N, 8, color::RED);
        rbtree_node_assert(&vec[10], N, N, 8, color::RED);

        // +------------------+      +------------------+      +------------------+      +------------------+
        // |       4b         |      |       4b         |      |       4r*        |      |       4b         |
        // |    /     \       |      |    /     \       |      |    /     \       |      |    /     \       |
        // |   2r     6r      |      |   2r     6r      |      |   2b     6b      |      |   2b     6b      |
        // |  /  \   /  \     |      |  /  \   /  \     |      |  /  \   /  \     |      |  /  \   /  \     |
        // | 0b  3b 5b  8b    | ---> | 0b  3b 5b  8r*   | ---> | 0b  3b 5b  8r    | ---> | 0b  3b 5b  8r    |
        // |  \         / \   |      |  \         / \   |      |  \         / \   |      |  \         / \   |
        // |   1r     7r  10r |      |   1r     7b  10b |      |   1r     7b  10b |      |   1r     7b  10b |
        // |              /   |      |              /   |      |              /   |      |              /   |
        // |            9r*   |      |            9r    |      |            9r    |      |            9r    |
        // +------------------+      +------------------+      +------------------+      +------------------+
        vec_rbtree_mut!(vec).insert_left(&mut root, 10, 9);
        assert_eq!(root, 4);
        rbtree_node_assert(&vec[4], 2, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::BLACK);
        rbtree_node_assert(&vec[0], N, 1, 2, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[8], 7, 10, 6, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);
        rbtree_node_assert(&vec[10], 9, N, 8, color::BLACK);
        rbtree_node_assert(&vec[9], N, N, 10, color::RED);
    }

    #[test]
    fn test_tree_traversal() {
        let mut vec: Vec<RBTreeNode> = Vec::new();
        vec.resize_with(11, RBTreeNode::new);

        #[allow(non_snake_case)]
        let N = usize::nullptr(); // For convenience.

        // +------------------+
        // |       4b         |
        // |    /     \       |
        // |   2b     6b      |
        // |  /  \   /  \     |
        // | 0b  3b 5b  8r    |
        // |  \         / \   |
        // |   1r     7b  10b |
        // |              /   |
        // |            9r    |
        // +------------------+
        rbtree_node_make(&mut vec[4], 2, 6, 4, color::BLACK);
        rbtree_node_make(&mut vec[2], 0, 3, 4, color::BLACK);
        rbtree_node_make(&mut vec[0], N, 1, 2, color::BLACK);
        rbtree_node_make(&mut vec[1], N, N, 0, color::RED);
        rbtree_node_make(&mut vec[3], N, N, 2, color::BLACK);
        rbtree_node_make(&mut vec[6], 5, 8, 4, color::BLACK);
        rbtree_node_make(&mut vec[5], N, N, 6, color::BLACK);
        rbtree_node_make(&mut vec[8], 7, 10, 6, color::RED);
        rbtree_node_make(&mut vec[7], N, N, 8, color::BLACK);
        rbtree_node_make(&mut vec[10], 9, N, 8, color::BLACK);
        rbtree_node_make(&mut vec[9], N, N, 10, color::RED);

        assert_eq!(vec_rbtree!(&vec).leftmost(0), 0);
        assert_eq!(vec_rbtree!(&vec).leftmost(1), 1);
        assert_eq!(vec_rbtree!(&vec).leftmost(2), 0);
        assert_eq!(vec_rbtree!(&vec).leftmost(3), 3);
        assert_eq!(vec_rbtree!(&vec).leftmost(4), 0);
        assert_eq!(vec_rbtree!(&vec).leftmost(5), 5);
        assert_eq!(vec_rbtree!(&vec).leftmost(6), 5);
        assert_eq!(vec_rbtree!(&vec).leftmost(7), 7);
        assert_eq!(vec_rbtree!(&vec).leftmost(8), 7);
        assert_eq!(vec_rbtree!(&vec).leftmost(9), 9);
        assert_eq!(vec_rbtree!(&vec).leftmost(10), 9);

        assert_eq!(vec_rbtree!(&vec).rightmost(0), 1);
        assert_eq!(vec_rbtree!(&vec).rightmost(1), 1);
        assert_eq!(vec_rbtree!(&vec).rightmost(2), 3);
        assert_eq!(vec_rbtree!(&vec).rightmost(3), 3);
        assert_eq!(vec_rbtree!(&vec).rightmost(4), 10);
        assert_eq!(vec_rbtree!(&vec).rightmost(5), 5);
        assert_eq!(vec_rbtree!(&vec).rightmost(6), 10);
        assert_eq!(vec_rbtree!(&vec).rightmost(7), 7);
        assert_eq!(vec_rbtree!(&vec).rightmost(8), 10);
        assert_eq!(vec_rbtree!(&vec).rightmost(9), 9);
        assert_eq!(vec_rbtree!(&vec).rightmost(10), 10);

        assert_eq!(vec_rbtree!(&vec).successor(0), 1);
        assert_eq!(vec_rbtree!(&vec).successor(1), 2);
        assert_eq!(vec_rbtree!(&vec).successor(2), 3);
        assert_eq!(vec_rbtree!(&vec).successor(3), 4);
        assert_eq!(vec_rbtree!(&vec).successor(4), 5);
        assert_eq!(vec_rbtree!(&vec).successor(5), 6);
        assert_eq!(vec_rbtree!(&vec).successor(6), 7);
        assert_eq!(vec_rbtree!(&vec).successor(7), 8);
        assert_eq!(vec_rbtree!(&vec).successor(8), 9);
        assert_eq!(vec_rbtree!(&vec).successor(9), 10);
        assert_eq!(vec_rbtree!(&vec).successor(10), N);

        assert_eq!(vec_rbtree!(&vec).predecessor(10), 9);
        assert_eq!(vec_rbtree!(&vec).predecessor(9), 8);
        assert_eq!(vec_rbtree!(&vec).predecessor(8), 7);
        assert_eq!(vec_rbtree!(&vec).predecessor(7), 6);
        assert_eq!(vec_rbtree!(&vec).predecessor(6), 5);
        assert_eq!(vec_rbtree!(&vec).predecessor(5), 4);
        assert_eq!(vec_rbtree!(&vec).predecessor(4), 3);
        assert_eq!(vec_rbtree!(&vec).predecessor(3), 2);
        assert_eq!(vec_rbtree!(&vec).predecessor(2), 1);
        assert_eq!(vec_rbtree!(&vec).predecessor(1), 0);
        assert_eq!(vec_rbtree!(&vec).predecessor(0), N);
    }

    #[test]
    fn test_swap() {
        let mut vec: Vec<RBTreeNode> = Vec::new();
        vec.resize_with(10, RBTreeNode::new);

        #[allow(non_snake_case)]
        let N = usize::nullptr(); // For convenience.

        // +---------------+
        // |       4b      |
        // |    /     \    |
        // |   2b     6b   |
        // |  /  \   /  \  |
        // | 0b  3b 5b  8r |
        // |  \         /  |
        // |   1r     7b   |
        // +---------------+
        rbtree_node_make(&mut vec[4], 2, 6, 4, color::BLACK);
        rbtree_node_make(&mut vec[2], 0, 3, 4, color::BLACK);
        rbtree_node_make(&mut vec[0], N, 1, 2, color::BLACK);
        rbtree_node_make(&mut vec[1], N, N, 0, color::RED);
        rbtree_node_make(&mut vec[3], N, N, 2, color::BLACK);
        rbtree_node_make(&mut vec[6], 5, 8, 4, color::BLACK);
        rbtree_node_make(&mut vec[5], N, N, 6, color::BLACK);
        rbtree_node_make(&mut vec[8], 7, N, 6, color::RED);
        rbtree_node_make(&mut vec[7], N, N, 8, color::BLACK);

        // +---------------+
        // |       4b      |
        // |    /     \    |
        // |   2b     6b   |
        // |  /  \   /  \  |
        // | 1b* 3b 5b  8r |
        // |  \         /  |
        // |   0r*    7b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(0, 1);
        rbtree_node_assert(&vec[4], 2, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 1, 3, 4, color::BLACK);
        rbtree_node_assert(&vec[1], N, 0, 2, color::BLACK);
        rbtree_node_assert(&vec[0], N, N, 1, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 6, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);

        // +---------------+
        // |       4b      |
        // |    /     \    |
        // |   3b*    6b   |
        // |  /  \   /  \  |
        // | 1b  2b*5b  8r |
        // |  \         /  |
        // |   0r     7b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(2, 3);
        rbtree_node_assert(&vec[4], 3, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[3], 1, 2, 4, color::BLACK);
        rbtree_node_assert(&vec[1], N, 0, 3, color::BLACK);
        rbtree_node_assert(&vec[0], N, N, 1, color::RED);
        rbtree_node_assert(&vec[2], N, N, 3, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 6, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);

        // +---------------+
        // |       4b      |
        // |    /     \    |
        // |   3b     6b   |
        // |  /  \   /  \  |
        // | 0b* 2b 5b  8r |
        // |  \         /  |
        // |   1r*    7b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(0, 1);
        rbtree_node_assert(&vec[4], 3, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[3], 0, 2, 4, color::BLACK);
        rbtree_node_assert(&vec[0], N, 1, 3, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[2], N, N, 3, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 6, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);

        // +---------------+
        // |       4b      |
        // |    /     \    |
        // |   2b*    6b   |
        // |  /  \   /  \  |
        // | 0b  3b*5b  8r |
        // |  \         /  |
        // |   1r     7b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(2, 3);
        rbtree_node_assert(&vec[4], 2, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::BLACK);
        rbtree_node_assert(&vec[0], N, 1, 2, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 6, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);

        // +----------------+
        // |       4b       |
        // |    /     \     |
        // |   2b     6b    |
        // |  /  \   /  \   |
        // | 0b  3b 5b  7r* |
        // |  \         /   |
        // |   1r     8b*   |
        // +----------------+
        vec_rbtree_mut!(vec).swap(7, 8);
        rbtree_node_assert(&vec[4], 2, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::BLACK);
        rbtree_node_assert(&vec[0], N, 1, 2, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 7, 4, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[7], 8, N, 6, color::RED);
        rbtree_node_assert(&vec[8], N, N, 7, color::BLACK);

        // +---------------+
        // |       4b      |
        // |    /     \    |
        // |   2b     5b*  |
        // |  /  \   /  \  |
        // | 0b  3b 6b* 7r |
        // |  \         /  |
        // |   1r     8b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(5, 6);
        rbtree_node_assert(&vec[4], 2, 5, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::BLACK);
        rbtree_node_assert(&vec[0], N, 1, 2, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[5], 6, 7, 4, color::BLACK);
        rbtree_node_assert(&vec[6], N, N, 5, color::BLACK);
        rbtree_node_assert(&vec[7], 8, N, 5, color::RED);
        rbtree_node_assert(&vec[8], N, N, 7, color::BLACK);

        // +----------------+
        // |       4b       |
        // |    /     \     |
        // |   2b     5b    |
        // |  /  \   /  \   |
        // | 0b  3b 6b  8r* |
        // |  \         /   |
        // |   1r     7b*   |
        // +----------------+
        vec_rbtree_mut!(vec).swap(7, 8);
        rbtree_node_assert(&vec[4], 2, 5, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::BLACK);
        rbtree_node_assert(&vec[0], N, 1, 2, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[5], 6, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[6], N, N, 5, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 5, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);

        // +---------------+
        // |       4b      |
        // |    /     \    |
        // |   2b     6b*  |
        // |  /  \   /  \  |
        // | 0b  3b 5b* 8r |
        // |  \         /  |
        // |   1r     7b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(5, 6);
        rbtree_node_assert(&vec[4], 2, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[2], 0, 3, 4, color::BLACK);
        rbtree_node_assert(&vec[0], N, 1, 2, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 2, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 6, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);

        // +---------------+
        // |       4b      |
        // |    /     \    |
        // |   1b*    6b   |
        // |  /  \   /  \  |
        // | 0b  3b 5b  8r |
        // |  \         /  |
        // |   2r*    7b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(1, 2);
        rbtree_node_assert(&vec[4], 1, 6, 4, color::BLACK);
        rbtree_node_assert(&vec[1], 0, 3, 4, color::BLACK);
        rbtree_node_assert(&vec[0], N, 2, 1, color::BLACK);
        rbtree_node_assert(&vec[2], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 1, color::BLACK);
        rbtree_node_assert(&vec[6], 5, 8, 4, color::BLACK);
        rbtree_node_assert(&vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 6, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);

        // +---------------+
        // |      5b*      |
        // |    /     \    |
        // |   1b     6b   |
        // |  /  \   /  \  |
        // | 0b  3b 4b* 8r |
        // |  \         /  |
        // |   2r     7b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(4, 5);
        rbtree_node_assert(&vec[5], 1, 6, 5, color::BLACK);
        rbtree_node_assert(&vec[1], 0, 3, 5, color::BLACK);
        rbtree_node_assert(&vec[0], N, 2, 1, color::BLACK);
        rbtree_node_assert(&vec[2], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 1, color::BLACK);
        rbtree_node_assert(&vec[6], 4, 8, 5, color::BLACK);
        rbtree_node_assert(&vec[4], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 6, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);

        // +---------------+
        // |      6b*      |
        // |    /     \    |
        // |   1b     5b*  |
        // |  /  \   /  \  |
        // | 0b  3b 4b  8r |
        // |  \         /  |
        // |   2r     7b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(5, 6);
        rbtree_node_assert(&vec[6], 1, 5, 6, color::BLACK);
        rbtree_node_assert(&vec[1], 0, 3, 6, color::BLACK);
        rbtree_node_assert(&vec[0], N, 2, 1, color::BLACK);
        rbtree_node_assert(&vec[2], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 1, color::BLACK);
        rbtree_node_assert(&vec[5], 4, 8, 6, color::BLACK);
        rbtree_node_assert(&vec[4], N, N, 5, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 5, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);

        // +---------------+
        // |      1b*      |
        // |    /     \    |
        // |   6b*    5b   |
        // |  /  \   /  \  |
        // | 0b  3b 4b  8r |
        // |  \         /  |
        // |   2r     7b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(1, 6);
        rbtree_node_assert(&vec[1], 6, 5, 1, color::BLACK);
        rbtree_node_assert(&vec[6], 0, 3, 1, color::BLACK);
        rbtree_node_assert(&vec[0], N, 2, 6, color::BLACK);
        rbtree_node_assert(&vec[2], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 6, color::BLACK);
        rbtree_node_assert(&vec[5], 4, 8, 1, color::BLACK);
        rbtree_node_assert(&vec[4], N, N, 5, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 5, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);

        // +---------------+
        // |      1b       |
        // |    /     \    |
        // |   9b*    5b   |
        // |  /  \   /  \  |
        // | 0b  3b 4b  8r |
        // |  \         /  |
        // |   2r     7b   |
        // +---------------+
        rbtree_node_assert(&vec[9], N, N, N, color::BLACK);
        vec_rbtree_mut!(vec).swap(9, 6);
        rbtree_node_assert(&vec[1], 9, 5, 1, color::BLACK);
        rbtree_node_assert(&vec[9], 0, 3, 1, color::BLACK);
        rbtree_node_assert(&vec[0], N, 2, 9, color::BLACK);
        rbtree_node_assert(&vec[2], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 9, color::BLACK);
        rbtree_node_assert(&vec[5], 4, 8, 1, color::BLACK);
        rbtree_node_assert(&vec[4], N, N, 5, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 5, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);
        rbtree_node_assert(&vec[6], N, N, N, color::BLACK);

        // +---------------+
        // |      6b*      |
        // |    /     \    |
        // |   9b     5b   |
        // |  /  \   /  \  |
        // | 0b  3b 4b  8r |
        // |  \         /  |
        // |   2r     7b   |
        // +---------------+
        vec_rbtree_mut!(vec).swap(1, 6);
        rbtree_node_assert(&vec[6], 9, 5, 6, color::BLACK);
        rbtree_node_assert(&vec[9], 0, 3, 6, color::BLACK);
        rbtree_node_assert(&vec[0], N, 2, 9, color::BLACK);
        rbtree_node_assert(&vec[2], N, N, 0, color::RED);
        rbtree_node_assert(&vec[3], N, N, 9, color::BLACK);
        rbtree_node_assert(&vec[5], 4, 8, 6, color::BLACK);
        rbtree_node_assert(&vec[4], N, N, 5, color::BLACK);
        rbtree_node_assert(&vec[8], 7, N, 5, color::RED);
        rbtree_node_assert(&vec[7], N, N, 8, color::BLACK);
        rbtree_node_assert(&vec[1], N, N, N, color::BLACK);
    }

    #[test]
    fn test_delete_1() {
        let mut vec: Vec<RBTreeNode> = Vec::new();
        vec.resize_with(11, RBTreeNode::new);

        #[allow(non_snake_case)]
        let N = usize::nullptr(); // For convenience.

        // +--------------------+
        // |      3b            |
        // |    /     \         |
        // |   1b      7b       |
        // |  /  \   /    \     |
        // | 0b  2b 5r     8r   |
        // |       /  \   /  \  |
        // |      4b  6b 9b 10b |
        // +--------------------+
        rbtree_node_make(&mut vec[3], 1, 7, 3, color::BLACK);
        rbtree_node_make(&mut vec[1], 0, 2, 3, color::BLACK);
        rbtree_node_make(&mut vec[0], N, N, 1, color::BLACK);
        rbtree_node_make(&mut vec[2], N, N, 1, color::BLACK);
        rbtree_node_make(&mut vec[7], 5, 8, 3, color::BLACK);
        rbtree_node_make(&mut vec[5], 4, 6, 7, color::RED);
        rbtree_node_make(&mut vec[4], N, N, 5, color::BLACK);
        rbtree_node_make(&mut vec[6], N, N, 5, color::BLACK);
        rbtree_node_make(&mut vec[8], 9, 10, 7, color::RED);
        rbtree_node_make(&mut vec[9], N, N, 8, color::BLACK);
        rbtree_node_make(&mut vec[10], N, N, 8, color::BLACK);
        let mut root = 3;

        // +--------------------+      +--------------------+      +--------------------+
        // |      3b            |      |      3b            |      |      3b            |
        // |    /     \         |      |    /     \         |      |    /     \         |
        // |   1b*     7b       |      |   2b     7b        |      |   2b*     7b       |
        // |  /  \   /    \     | ---> |  /  \   /    \     | ---> |  /      /    \     | --->
        // | 0b  2b 5r     8r   |      | 0b 1b* 5r     8r   |      | 0r     5r     8r   |
        // |       /  \   /  \  |      |       /  \   /  \  |      |       /  \   /  \  |
        // |      4b  6b 9b 10b |      |      4b  6b 9b 10b |      |      4b  6b 9b 10b |
        // +--------------------+      +--------------------+      +--------------------+
        // +-----------------+      +-----------------+
        // |        7b       |      |        7b       |
        // |      /     \    |      |      /     \    |
        // |     3b     8r   |      |     3b     8b   |
        // |    /  \   /  \  | ---> |    /  \   /  \  |
        // |   2b* 5r 9b 10b |      |   2b  5r 9b 10b |
        // |  /   /  \       |      |  /   /  \       |
        // | 0r  4b  6b      |      | 0r  4b  6b      |
        // +-----------------+      +-----------------+
        vec_rbtree_mut!(vec).delete(&mut root, 1);
        assert_eq!(root, 7);
        rbtree_node_assert(&mut vec[7], 3, 8, 7, color::BLACK);
        rbtree_node_assert(&mut vec[3], 2, 5, 7, color::BLACK);
        rbtree_node_assert(&mut vec[2], 0, N, 3, color::BLACK);
        rbtree_node_assert(&mut vec[0], N, N, 2, color::RED);
        rbtree_node_assert(&mut vec[5], 4, 6, 3, color::RED);
        rbtree_node_assert(&mut vec[4], N, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[6], N, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[8], 9, 10, 7, color::BLACK);
        rbtree_node_assert(&mut vec[9], N, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[1], N, N, N, color::BLACK);

        // +-----------------+      +-----------------+      +---------------+
        // |        7b       |      |        7b       |      |      7b       |
        // |      /     \    |      |      /     \    |      |    /     \    |
        // |     3b     8b   |      |     3b     8b   |      |   3b     8b   |
        // |    /  \   /  \  | ---> |    /  \   /  \  | ---> |  /  \   /  \  |
        // |   2b* 5r 9b 10b |      |   2r* 5r 9b 10b |      | 0b  5r 9b 10b |
        // |  /   /  \       |      |  /   /  \       |      |    /  \       |
        // | 0r  4b  6b      |      | 0b  4b  6b      |      |   4b  6b      |
        // +-----------------+      +-----------------+      +---------------+
        vec_rbtree_mut!(vec).delete(&mut root, 2);
        assert_eq!(root, 7);
        rbtree_node_assert(&mut vec[7], 3, 8, 7, color::BLACK);
        rbtree_node_assert(&mut vec[3], 0, 5, 7, color::BLACK);
        rbtree_node_assert(&mut vec[0], N, N, 3, color::BLACK);
        rbtree_node_assert(&mut vec[5], 4, 6, 3, color::RED);
        rbtree_node_assert(&mut vec[4], N, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[6], N, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[8], 9, 10, 7, color::BLACK);
        rbtree_node_assert(&mut vec[9], N, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[2], N, N, N, color::BLACK);

        // +---------------+      +-----------------+      +-----------------+      +---------------+
        // |      7b       |      |        7b       |      |        7b       |      |      7b       |
        // |    /     \    |      |      /     \    |      |      /     \    |      |    /     \    |
        // |   3b     8b   |      |     5r     8b   |      |     5b     8b   |      |   5b     8b   |
        // |  /  \   /  \  | ---> |    /  \   /  \  | ---> |    /  \   /  \  | ---> |  /  \   /  \  |
        // | 0b* 5r 9b 10b |      |   3b  6b 9b 10b |      |   3r  6b 9b 10b |      | 3b  6b 9b 10b |
        // |    /  \       |      |  /  \           |      |  / \            |      |  \            |
        // |   4b  6b      |      | 0b* 4b          |      | 0b* 4b          |      |   4r          |
        // +---------------+      +-----------------+      +-----------------+      +---------------+
        vec_rbtree_mut!(vec).delete(&mut root, 0);
        assert_eq!(root, 7);
        rbtree_node_assert(&mut vec[7], 5, 8, 7, color::BLACK);
        rbtree_node_assert(&mut vec[5], 3, 6, 7, color::BLACK);
        rbtree_node_assert(&mut vec[3], N, 4, 5, color::BLACK);
        rbtree_node_assert(&mut vec[4], N, N, 3, color::RED);
        rbtree_node_assert(&mut vec[6], N, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[8], 9, 10, 7, color::BLACK);
        rbtree_node_assert(&mut vec[9], N, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[0], N, N, N, color::BLACK);

        // +---------------+      +---------------+      +-----------------+
        // |      7b       |      |      7b       |      |        7b       |
        // |    /     \    |      |    /     \    |      |      /     \    |
        // |   5b*    8b   |      |   6b     8b   |      |     6b     8b   |
        // |  /  \   /  \  | ---> |  /  \   /  \  | ---> |    /  \   /  \  | --->
        // | 3b  6b 9b 10b |      | 3b 5b* 9b 10b |      |   4b 5b* 9b 10b |
        // |  \            |      |  \            |      |  /              |
        // |   4r          |      |   4r          |      | 3r              |
        // +---------------+      +---------------+      +-----------------+
        // +---------------+      +---------------+
        // |      7b       |      |      7b       |
        // |    /     \    |      |    /     \    |
        // |   4b     8b   |      |   4b     8b   |
        // |  /  \   /  \  | ---> |  /  \   /  \  |
        // | 3r  6b 9b 10b |      | 3b  6b 9b 10b |
        // |       \       |      +---------------+
        // |       5b*     |
        // +---------------+
        vec_rbtree_mut!(vec).delete(&mut root, 5);
        assert_eq!(root, 7);
        rbtree_node_assert(&mut vec[7], 4, 8, 7, color::BLACK);
        rbtree_node_assert(&mut vec[4], 3, 6, 7, color::BLACK);
        rbtree_node_assert(&mut vec[3], N, N, 4, color::BLACK);
        rbtree_node_assert(&mut vec[6], N, N, 4, color::BLACK);
        rbtree_node_assert(&mut vec[8], 9, 10, 7, color::BLACK);
        rbtree_node_assert(&mut vec[9], N, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[5], N, N, N, color::BLACK);

        // +---------------+      +---------------+      +---------------+      +--------------+
        // |      7b*      |      |      9b       |      |      9b       |      |      9b      |
        // |    /     \    |      |    /     \    |      |    /     \    |      |    /    \    |
        // |   4b     8b   | ---> |   4b     8b   | ---> |   4b     8b*  | ---> |   4r    8b   |
        // |  /  \   /  \  |      |  /  \   /  \  |      |  /  \      \  |      |  /  \     \  |
        // | 3b  6b 9b 10b |      | 3b  6b 7b*10b |      | 3b  6b    10r |      | 3b  6b   10r |
        // +---------------+      +---------------+      +---------------+      +--------------+
        vec_rbtree_mut!(vec).delete(&mut root, 7);
        assert_eq!(root, 9);
        rbtree_node_assert(&mut vec[9], 4, 8, 9, color::BLACK);
        rbtree_node_assert(&mut vec[4], 3, 6, 9, color::RED);
        rbtree_node_assert(&mut vec[3], N, N, 4, color::BLACK);
        rbtree_node_assert(&mut vec[6], N, N, 4, color::BLACK);
        rbtree_node_assert(&mut vec[8], N, 10, 9, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 8, color::RED);
        rbtree_node_assert(&mut vec[7], N, N, N, color::BLACK);

        // +--------------+      +--------------+      +--------------+      +--------------+
        // |      9b      |      |      9b      |      |      9b      |      |      9b      |
        // |    /    \    |      |    /    \    |      |    /    \    |      |    /    \    |
        // |   4r*   8b   | ---> |   6r    8b   | ---> |   6b    8b   | ---> |   6b    8b   |
        // |  /  \     \  |      |  /  \     \  |      |  /  \     \  |      |  /        \  |
        // | 3b  6b   10r |      | 3b  4b*  10r |      | 3r  4b*  10r |      | 3r       10r |
        // +--------------+      +--------------+      +--------------+      +--------------+
        vec_rbtree_mut!(vec).delete(&mut root, 4);
        assert_eq!(root, 9);
        rbtree_node_assert(&mut vec[9], 6, 8, 9, color::BLACK);
        rbtree_node_assert(&mut vec[6], 3, N, 9, color::BLACK);
        rbtree_node_assert(&mut vec[3], N, N, 6, color::RED);
        rbtree_node_assert(&mut vec[8], N, 10, 9, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 8, color::RED);
        rbtree_node_assert(&mut vec[4], N, N, N, color::BLACK);

        // +--------------+      +--------------+      +-----------+
        // |      9b*     |      |      8b      |      |     8b    |
        // |    /    \    |      |    /    \    |      |    /  \   |
        // |   6b    8b   | ---> |   6b    9b*  | ---> |   6b  10b |
        // |  /        \  |      |  /        \  |      |  /        |
        // | 3r       10r |      | 3r       10r |      | 3r        |
        // +--------------+      +--------------+      +-----------+
        vec_rbtree_mut!(vec).delete(&mut root, 9);
        assert_eq!(root, 8);
        rbtree_node_assert(&mut vec[8], 6, 10, 8, color::BLACK);
        rbtree_node_assert(&mut vec[6], 3, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[3], N, N, 6, color::RED);
        rbtree_node_assert(&mut vec[10], N, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[9], N, N, N, color::BLACK);

        // +-----------+      +----------+      +--------+
        // |     8b    |      |   6b     |      |   6b   |
        // |    /  \   |      |  /  \    |      |  /  \  |
        // |   6b 10b* | ---> | 3r  8b   | ---> | 3b  8b |
        // |  /        |      |       \  |      +--------+
        // | 3r        |      |     10b* |
        // +-----------+      +----------+
        vec_rbtree_mut!(vec).delete(&mut root, 10);
        assert_eq!(root, 6);
        rbtree_node_assert(&mut vec[6], 3, 8, 6, color::BLACK);
        rbtree_node_assert(&mut vec[3], N, N, 6, color::BLACK);
        rbtree_node_assert(&mut vec[8], N, N, 6, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, N, color::BLACK);

        // +--------+      +--------+      +------+
        // |   6b*  |      |   8b   |      |   8b |
        // |  /  \  | ---> |  /  \  | ---> |  /   |
        // | 3b  8b |      | 3b 6b* |      | 3r   |
        // +--------+      +--------+      +------+
        vec_rbtree_mut!(vec).delete(&mut root, 6);
        assert_eq!(root, 8);
        rbtree_node_assert(&mut vec[8], 3, N, 8, color::BLACK);
        rbtree_node_assert(&mut vec[3], N, N, 8, color::RED);
        rbtree_node_assert(&mut vec[6], N, N, N, color::BLACK);

        // +-------+      +-------+
        // |   8b* |      |   8r* |      +----+
        // |  /    | ---> |  /    | ---> | 3b |
        // | 3r    |      | 3b    |      +----+
        // +-------+      +-------+
        vec_rbtree_mut!(vec).delete(&mut root, 8);
        assert_eq!(root, 3);
        rbtree_node_assert(&mut vec[3], N, N, 3, color::BLACK);
        rbtree_node_assert(&mut vec[8], N, N, N, color::BLACK);

        // 3b* ---> *
        vec_rbtree_mut!(vec).delete(&mut root, 3);
        assert_eq!(root, N);
        rbtree_node_assert(&mut vec[3], N, N, N, color::BLACK);
    }

    #[test]
    fn test_delete_2() {
        let mut vec: Vec<RBTreeNode> = Vec::new();
        vec.resize_with(11, RBTreeNode::new);

        #[allow(non_snake_case)]
        let N = usize::nullptr(); // For convenience.

        // +--------------------+
        // |          7b        |
        // |       /      \     |
        // |      3b       9b   |
        // |    /    \    /  \  |
        // |   1r     5r 8b 10b |
        // |  /  \   /  \       |
        // | 0b  2b 4b  6b      |
        // +--------------------+
        rbtree_node_make(&mut vec[7], 3, 9, 7, color::BLACK);
        rbtree_node_make(&mut vec[3], 1, 5, 7, color::BLACK);
        rbtree_node_make(&mut vec[1], 0, 2, 3, color::RED);
        rbtree_node_make(&mut vec[0], N, N, 1, color::BLACK);
        rbtree_node_make(&mut vec[2], N, N, 1, color::BLACK);
        rbtree_node_make(&mut vec[5], 4, 6, 3, color::RED);
        rbtree_node_make(&mut vec[4], N, N, 5, color::BLACK);
        rbtree_node_make(&mut vec[6], N, N, 5, color::BLACK);
        rbtree_node_make(&mut vec[9], 8, 10, 7, color::BLACK);
        rbtree_node_make(&mut vec[8], N, N, 9, color::BLACK);
        rbtree_node_make(&mut vec[10], N, N, 9, color::BLACK);
        let mut root = 7;

        // +--------------------+      +--------------------+      +------------------+      +------------------+
        // |          7b        |      |          7b        |      |       3b         |      |       3b         |
        // |       /      \     |      |       /      \     |      |    /      \      |      |    /      \      |
        // |      3b       9b   |      |      3b       9b*  |      |   1r      7b     |      |   1b      7b     |
        // |    /    \    /  \  | ---> |    /    \       \  | ---> |  /  \   /   \    | ---> |  /  \   /   \    |
        // |   1r     5r 8b*10b |      |   1r     5r    10r |      | 0b  2b 5r    9b* |      | 0b  2b 5r    9b  |
        // |  /  \   /  \       |      |  /  \   /  \       |      |       /  \    \  |      |       /  \    \  |
        // | 0b  2b 4b  6b      |      | 0b  2b 4b  6b      |      |      4b  6b  10r |      |      4b  6b  10r |
        // +--------------------+      +--------------------+      +------------------+      +------------------+
        vec_rbtree_mut!(vec).delete(&mut root, 8);
        assert_eq!(root, 3);
        rbtree_node_assert(&mut vec[3], 1, 7, 3, color::BLACK);
        rbtree_node_assert(&mut vec[1], 0, 2, 3, color::BLACK);
        rbtree_node_assert(&mut vec[0], N, N, 1, color::BLACK);
        rbtree_node_assert(&mut vec[2], N, N, 1, color::BLACK);
        rbtree_node_assert(&mut vec[7], 5, 9, 3, color::BLACK);
        rbtree_node_assert(&mut vec[5], 4, 6, 7, color::RED);
        rbtree_node_assert(&mut vec[4], N, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[6], N, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[9], N, 10, 7, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 9, color::RED);
        rbtree_node_assert(&mut vec[8], N, N, N, color::BLACK);

        // +------------------+      +------------------+      +----------------+
        // |       3b         |      |       3b         |      |       3b       |
        // |    /      \      |      |    /      \      |      |    /      \    |
        // |   1b      7b     |      |   1b      7b     |      |   1b      7b   |
        // |  /  \   /   \    | ---> |  /  \   /   \    | ---> |  /  \   /   \  |
        // | 0b  2b 5r   9b*  |      | 0b  2b 5r   9r*  |      | 0b  2b 5r  10b |
        // |       /  \    \  |      |       /  \    \  |      |       /  \     |
        // |      4b  6b  10r |      |      4b  6b  10b |      |      4b  6b    |
        // +------------------+      +------------------+      +----------------+
        vec_rbtree_mut!(vec).delete(&mut root, 9);
        assert_eq!(root, 3);
        rbtree_node_assert(&mut vec[3], 1, 7, 3, color::BLACK);
        rbtree_node_assert(&mut vec[1], 0, 2, 3, color::BLACK);
        rbtree_node_assert(&mut vec[0], N, N, 1, color::BLACK);
        rbtree_node_assert(&mut vec[2], N, N, 1, color::BLACK);
        rbtree_node_assert(&mut vec[7], 5, 10, 3, color::BLACK);
        rbtree_node_assert(&mut vec[5], 4, 6, 7, color::RED);
        rbtree_node_assert(&mut vec[4], N, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[6], N, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 7, color::BLACK);
        rbtree_node_assert(&mut vec[9], N, N, N, color::BLACK);

        // +----------------+      +----------------+      +-------------------+
        // |       3b       |      |       3b       |      |       3b          |
        // |    /      \    |      |    /      \    |      |    /      \       |
        // |   1b     7b*   |      |   1b     10b   |      |   1b      5r      |
        // |  /  \   /   \  | ---> |  /  \   /   \  | ---> |  /  \   /   \     | --->
        // | 0b  2b 5r  10b |      | 0b  2b 5r  7b* |      | 0b  2b 4b  10b    |
        // |       /  \     |      |       /  \     |      |            /  \   |
        // |      4b  6b    |      |      4b  6b    |      |           6b  7b* |
        // +----------------+      +----------------+      +-------------------+
        // +-------------------+      +----------------+
        // |       3b          |      |       3b       |
        // |    /      \       |      |    /      \    |
        // |   1b      5b      |      |   1b      5b   |
        // |  /  \   /   \     | ---> |  /  \   /   \  |
        // | 0b  2b 4b  10r    |      | 0b  2b 4b  10b |
        // |            /  \   |      |             /  |
        // |           6b  7b* |      |            6r  |
        // +-------------------+      +----------------+
        vec_rbtree_mut!(vec).delete(&mut root, 7);
        assert_eq!(root, 3);
        rbtree_node_assert(&mut vec[3], 1, 5, 3, color::BLACK);
        rbtree_node_assert(&mut vec[1], 0, 2, 3, color::BLACK);
        rbtree_node_assert(&mut vec[0], N, N, 1, color::BLACK);
        rbtree_node_assert(&mut vec[2], N, N, 1, color::BLACK);
        rbtree_node_assert(&mut vec[5], 4, 10, 3, color::BLACK);
        rbtree_node_assert(&mut vec[4], N, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[10], 6, N, 5, color::BLACK);
        rbtree_node_assert(&mut vec[6], N, N, 10, color::RED);
        rbtree_node_assert(&mut vec[7], N, N, N, color::BLACK);

        // +----------------+      +-----------------+      +---------------+      +---------------+
        // |       3b       |      |      3b         |      |      3b       |      |      3b       |
        // |    /      \    |      |    /     \      |      |    /     \    |      |    /     \    |
        // |   1b      5b   |      |   1b     5b     |      |   1b     6b   |      |   1b     6b   |
        // |  /  \   /   \  | ---> |  /  \   /  \    | ---> |  /  \   /  \  | ---> |  /  \   /  \  |
        // | 0b  2b 4b* 10b |      | 0b  2b 4b* 6b   |      | 0b  2b 5b 10r |      | 0b  2b 5b 10b |
        // |             /  |      |              \  |      |       /       |      +---------------+
        // |            6r  |      |             10r |      |      4b*      |
        // +----------------+      +-----------------+      +---------------+
        vec_rbtree_mut!(vec).delete(&mut root, 4);
        assert_eq!(root, 3);
        rbtree_node_assert(&mut vec[3], 1, 6, 3, color::BLACK);
        rbtree_node_assert(&mut vec[1], 0, 2, 3, color::BLACK);
        rbtree_node_assert(&mut vec[0], N, N, 1, color::BLACK);
        rbtree_node_assert(&mut vec[2], N, N, 1, color::BLACK);
        rbtree_node_assert(&mut vec[6], 5, 10, 3, color::BLACK);
        rbtree_node_assert(&mut vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 6, color::BLACK);
        rbtree_node_assert(&mut vec[4], N, N, N, color::BLACK);

        // +---------------+      +---------------+      +------------+      +------------+
        // |      3b       |      |      3b       |      |     3b     |      |     3b     |
        // |    /     \    |      |    /     \    |      |    /  \    |      |    /  \    |
        // |   1b*    6b   | ---> |   2b     6b   | ---> |   2b* 6b   | ---> |   2b  6r   |
        // |  /  \   /  \  |      |  /  \   /  \  |      |  /   /  \  |      |  /   /  \  |
        // | 0b  2b 5b 10b |      | 0b 1b* 5b 10b |      | 0r  5b 10b |      | 0r  5b 10b |
        // +---------------+      +---------------+      +------------+      +------------+
        vec_rbtree_mut!(vec).delete(&mut root, 1);
        assert_eq!(root, 3);
        rbtree_node_assert(&mut vec[3], 2, 6, 3, color::BLACK);
        rbtree_node_assert(&mut vec[2], 0, N, 3, color::BLACK);
        rbtree_node_assert(&mut vec[0], N, N, 2, color::RED);
        rbtree_node_assert(&mut vec[6], 5, 10, 3, color::RED);
        rbtree_node_assert(&mut vec[5], N, N, 6, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 6, color::BLACK);
        rbtree_node_assert(&mut vec[1], N, N, N, color::BLACK);

        // +-------------+      +------------+
        // |     3b      |      |     3b     |
        // |    /  \     |      |    /  \    |
        // |   2b  6r    | ---> |   2b  6b   |
        // |  /   /  \   |      |  /      \  |
        // | 0r  5b* 10b |      | 0r     10r |
        // +-------------+      +------------+
        vec_rbtree_mut!(vec).delete(&mut root, 5);
        assert_eq!(root, 3);
        rbtree_node_assert(&mut vec[3], 2, 6, 3, color::BLACK);
        rbtree_node_assert(&mut vec[2], 0, N, 3, color::BLACK);
        rbtree_node_assert(&mut vec[0], N, N, 2, color::RED);
        rbtree_node_assert(&mut vec[6], N, 10, 3, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 6, color::RED);
        rbtree_node_assert(&mut vec[5], N, N, N, color::BLACK);

        // +------------+      +----------+
        // |     3b     |      |   3b     |
        // |    /  \    |      |  /  \    |
        // |   2b  6b   | ---> | 2b  6b   |
        // |  /      \  |      |       \  |
        // | 0r*    10r |      |      10r |
        // +------------+      +----------+
        vec_rbtree_mut!(vec).delete(&mut root, 0);
        assert_eq!(root, 3);
        rbtree_node_assert(&mut vec[3], 2, 6, 3, color::BLACK);
        rbtree_node_assert(&mut vec[2], N, N, 3, color::BLACK);
        rbtree_node_assert(&mut vec[6], N, 10, 3, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 6, color::RED);
        rbtree_node_assert(&mut vec[0], N, N, N, color::BLACK);

        // +----------+      +----------+      +--------+
        // |   3b     |      |     6b   |      |   6b   |
        // |  /  \    |      |    /  \  |      |  /  \  |
        // | 2b* 6b   | ---> |   3b 10r | ---> | 3b 10b |
        // |       \  |      |  /       |      +--------+
        // |      10r |      | 2b*      |
        // +----------+      +----------+
        vec_rbtree_mut!(vec).delete(&mut root, 2);
        assert_eq!(root, 6);
        rbtree_node_assert(&mut vec[6], 3, 10, 6, color::BLACK);
        rbtree_node_assert(&mut vec[3], N, N, 6, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 6, color::BLACK);
        rbtree_node_assert(&mut vec[2], N, N, N, color::BLACK);

        // +---------+      +------+
        // |   6b    |      | 6b   |
        // |  /  \   | ---> |   \  |
        // | 3b* 10b |      |  10r |
        // +---------+      +------+
        vec_rbtree_mut!(vec).delete(&mut root, 3);
        assert_eq!(root, 6);
        rbtree_node_assert(&mut vec[6], N, 10, 6, color::BLACK);
        rbtree_node_assert(&mut vec[10], N, N, 6, color::RED);
        rbtree_node_assert(&mut vec[3], N, N, N, color::BLACK);

        // +------+      +------+
        // | 6b*  |      | 6r*  |      +-----+
        // |   \  | ---> |   \  | ---> | 10b |
        // |  10r |      |  10b |      +-----+
        // +------+      +------+
        vec_rbtree_mut!(vec).delete(&mut root, 6);
        assert_eq!(root, 10);
        rbtree_node_assert(&mut vec[10], N, N, 10, color::BLACK);
        rbtree_node_assert(&mut vec[6], N, N, N, color::BLACK);

        // 10b* ---> *
        vec_rbtree_mut!(vec).delete(&mut root, 10);
        assert_eq!(root, N);
        rbtree_node_assert(&mut vec[10], N, N, N, color::BLACK);
    }

    struct RBTreeNodeIndexKeyVec {}

    impl RBTreeNodeIndex for RBTreeNodeIndexKeyVec {
        type Container = Vec<(RBTreeNode, usize)>;

        fn index(c: &Self::Container, i: usize) -> &RBTreeNode {
            &c[i].0
        }

        fn index_mut(c: &mut Self::Container, i: usize) -> &mut RBTreeNode {
            &mut c[i].0
        }
    }

    impl RBTreeNodeIndexKey for RBTreeNodeIndexKeyVec {
        type Key = usize;

        fn key(c: &Self::Container, i: usize) -> &Self::Key {
            &c[i].1
        }
    }

    macro_rules! kvec_rbtree {
        ($p: expr) => {
            RBTree::<_, _, RBTreeNodeIndexKeyVec>::new(&($p))
        };
    }

    macro_rules! kvec_rbtree_mut {
        ($p: expr) => {
            RBTreeMut::<_, _, RBTreeNodeIndexKeyVec>::new(&mut ($p))
        };
    }

    #[test]
    fn test_key_find_insert() {
        let mut kvec: Vec<(RBTreeNode, usize)> = Vec::new();
        let mut root = usize::nullptr();

        #[allow(non_snake_case)]
        let N = usize::nullptr(); // For convenience.

        // * -> 0b:3*
        kvec.push((RBTreeNode::new(), 3));
        let ret = kvec_rbtree_mut!(kvec).find_or_insert(&mut root, 0);
        assert_eq!(ret, N);
        rbtree_node_assert(&kvec[0].0, N, N, 0, color::BLACK);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 0);
        assert_eq!(kvec_rbtree!(kvec).find(root, &3), 0);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 0);
        assert_eq!(kvec_rbtree!(kvec).find(root, &2), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 0);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), N);

        // +---------+
        // | 0b:3    |
        // |   \     |
        // |   1r:3* |
        // +---------+
        kvec.push((RBTreeNode::new(), 3));
        let ret = kvec_rbtree_mut!(kvec).find_or_insert(&mut root, 1);
        assert_eq!(ret, 0);
        kvec_rbtree_mut!(kvec).insert(&mut root, 1);
        rbtree_node_assert(&kvec[0].0, N, 1, 0, color::BLACK);
        rbtree_node_assert(&kvec[1].0, N, N, 0, color::RED);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 0);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &3), 0);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &3), 1);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 0);
        assert_eq!(kvec_rbtree!(kvec).find(root, &2), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 1);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), N);

        // +-----------+      +-------------+
        // | 0b:3      |      |    1b:3     |
        // |   \       |      |    /  \     |
        // |   1r:3    | ---> | 0r:3   2r:3 |
        // |     \     |      +-------------+
        // |     2r:3* |
        // +-----------+
        kvec.push((RBTreeNode::new(), 3));
        kvec_rbtree_mut!(kvec).insert_last(&mut root, 2);
        rbtree_node_assert(&kvec[0].0, N, N, 1, color::RED);
        rbtree_node_assert(&kvec[1].0, 0, 2, 1, color::BLACK);
        rbtree_node_assert(&kvec[2].0, N, N, 1, color::RED);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 1);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &3), 0);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &3), 2);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 0);
        assert_eq!(kvec_rbtree!(kvec).find(root, &2), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 2);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), N);

        // +---------------+      +--------------+
        // |    1b:3       |      |    1b:3      |
        // |    /  \       |      |    /  \      |
        // | 0r:3   2r:3   | ---> | 0b:3   2b:3  |
        // |         \     |      |         \    |
        // |         3r:3* |      |         3r:3 |
        // +---------------+      +--------------+
        kvec.push((RBTreeNode::new(), 3));
        kvec_rbtree_mut!(kvec).insert_last(&mut root, 3);
        rbtree_node_assert(&kvec[0].0, N, N, 1, color::BLACK);
        rbtree_node_assert(&kvec[1].0, 0, 2, 1, color::BLACK);
        rbtree_node_assert(&kvec[2].0, N, 3, 1, color::BLACK);
        rbtree_node_assert(&kvec[3].0, N, N, 2, color::RED);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 1);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &3), 0);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &3), 3);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 0);
        assert_eq!(kvec_rbtree!(kvec).find(root, &2), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 3);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), N);

        // +----------------+
        // |      1b:3      |
        // |      /  \      |
        // |   0b:3   2b:3  |
        // |    /      \    |
        // | 4r:3*     3r:3 |
        // +----------------+
        kvec.push((RBTreeNode::new(), 3));
        kvec_rbtree_mut!(kvec).insert_first(&mut root, 4);
        rbtree_node_assert(&kvec[0].0, 4, N, 1, color::BLACK);
        rbtree_node_assert(&kvec[1].0, 0, 2, 1, color::BLACK);
        rbtree_node_assert(&kvec[2].0, N, 3, 1, color::BLACK);
        rbtree_node_assert(&kvec[3].0, N, N, 2, color::RED);
        rbtree_node_assert(&kvec[4].0, N, N, 0, color::RED);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 1);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &3), 4);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &3), 3);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 4);
        assert_eq!(kvec_rbtree!(kvec).find(root, &2), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 3);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), N);

        // +------------------+      +----------------+
        // |        1b:3      |      |      1b:3      |
        // |        /  \      |      |      /  \      |
        // |     0b:3   2b:3  |      |   4b:3   2b:3  |
        // |      /      \    | ---> |    / \    \    |
        // |   4r:3      3r:3 |      | 5r:2 0r:3 3r:3 |
        // |    /             |      +----------------+
        // | 5r:2*            |
        // +------------------+
        kvec.push((RBTreeNode::new(), 2));
        let ret = kvec_rbtree_mut!(kvec).find_or_insert(&mut root, 5);
        assert_eq!(ret, N);
        rbtree_node_assert(&kvec[0].0, N, N, 4, color::RED);
        rbtree_node_assert(&kvec[1].0, 4, 2, 1, color::BLACK);
        rbtree_node_assert(&kvec[2].0, N, 3, 1, color::BLACK);
        rbtree_node_assert(&kvec[3].0, N, N, 2, color::RED);
        rbtree_node_assert(&kvec[4].0, 5, 0, 1, color::BLACK);
        rbtree_node_assert(&kvec[5].0, N, N, 4, color::RED);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 1);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &3), 4);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &3), 3);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 5);
        assert_eq!(kvec_rbtree!(kvec).find(root, &2), 5);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &1), 5);
        assert_eq!(kvec_rbtree!(kvec).find(root, &1), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 3);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), N);

        // +----------------+      +----------------+
        // |      1b:3      |      |      1b:3      |
        // |      /  \      |      |      /  \      |
        // |   4b:3   2b:3  |      |   4r:3   2b:3  |
        // |    / \    \    |      |    / \    \    |
        // | 5r:2 0r:3 3r:3 | ---> | 5b:2 0b:3 3r:3 |
        // |    \           |      |    \           |
        // |   6r:3*        |      |   6r:3         |
        // +----------------+      +----------------+
        kvec.push((RBTreeNode::new(), 3));
        kvec_rbtree_mut!(kvec).insert_first(&mut root, 6);
        rbtree_node_assert(&kvec[0].0, N, N, 4, color::BLACK);
        rbtree_node_assert(&kvec[1].0, 4, 2, 1, color::BLACK);
        rbtree_node_assert(&kvec[2].0, N, 3, 1, color::BLACK);
        rbtree_node_assert(&kvec[3].0, N, N, 2, color::RED);
        rbtree_node_assert(&kvec[4].0, 5, 0, 1, color::RED);
        rbtree_node_assert(&kvec[5].0, N, 6, 4, color::BLACK);
        rbtree_node_assert(&kvec[6].0, N, N, 5, color::RED);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 1);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &3), 6);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &3), 3);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 5);
        assert_eq!(kvec_rbtree!(kvec).find(root, &2), 5);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &1), 5);
        assert_eq!(kvec_rbtree!(kvec).find(root, &1), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 3);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), N);

        // +----------------+      +----------------+      +------------------+
        // |      1b:3      |      |      1b:3      |      |        1b:3      |
        // |      /  \      |      |      /  \      |      |        /  \      |
        // |   4r:3   2b:3  |      |   4r:3   2b:3  |      |     4r:3   2b:3  |
        // |    / \    \    |      |    / \    \    |      |      / \    \    |
        // | 5b:2 0b:3 3r:3 | ---> | 5b:2 0b:3 3r:3 | ---> |   7b:2 0b:3 3r:3 |
        // |    \           |      |    \           |      |    / \           |
        // |   6r:3         |      |   7r:2*        |      | 5r:2 6r:3        |
        // |    /           |      |      \         |      +------------------+
        // | 7r:2*          |      |      6r:3      |
        // +----------------+      +----------------+
        kvec.push((RBTreeNode::new(), 2));
        kvec_rbtree_mut!(kvec).insert_last(&mut root, 7);
        rbtree_node_assert(&kvec[0].0, N, N, 4, color::BLACK);
        rbtree_node_assert(&kvec[1].0, 4, 2, 1, color::BLACK);
        rbtree_node_assert(&kvec[2].0, N, 3, 1, color::BLACK);
        rbtree_node_assert(&kvec[3].0, N, N, 2, color::RED);
        rbtree_node_assert(&kvec[4].0, 7, 0, 1, color::RED);
        rbtree_node_assert(&kvec[5].0, N, N, 7, color::RED);
        rbtree_node_assert(&kvec[6].0, N, N, 7, color::RED);
        rbtree_node_assert(&kvec[7].0, 5, 6, 4, color::BLACK);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 1);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &3), 6);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &3), 3);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 7);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &2), 5);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &2), 7);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &1), 5);
        assert_eq!(kvec_rbtree!(kvec).find(root, &1), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 3);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), N);

        // +------------------+      +------------------+      +-----------------------+
        // |        1b:3      |      |        1b:3      |      |         4b:3          |
        // |        /  \      |      |        /  \      |      |      /       \        |
        // |     4r:3   2b:3  |      |     4r:3   2b:3  |      |    7r:2     1r:3      |
        // |      / \    \    |      |      / \    \    |      |    /  \     /  \      |
        // |   7b:2 0b:3 3r:3 | ---> |  7r:2* 0b:3 3r:3 | ---> | 5b:2 6b:3 0b:3 2b:3   |
        // |    / \           |      |    / \           |      |       /          \    |
        // | 5r:2 6r:3        |      | 5b:2 6b:3        |      |    8r:3          3r:3 |
        // |      /           |      |      /           |      +-----------------------+
        // |   8r:3*          |      |   8r:3           |
        // +------------------+      +------------------+
        kvec.push((RBTreeNode::new(), 3));
        kvec_rbtree_mut!(kvec).insert_first(&mut root, 8);
        assert_eq!(root, 4);
        rbtree_node_assert(&kvec[0].0, N, N, 1, color::BLACK);
        rbtree_node_assert(&kvec[1].0, 0, 2, 4, color::RED);
        rbtree_node_assert(&kvec[2].0, N, 3, 1, color::BLACK);
        rbtree_node_assert(&kvec[3].0, N, N, 2, color::RED);
        rbtree_node_assert(&kvec[4].0, 7, 1, 4, color::BLACK);
        rbtree_node_assert(&kvec[5].0, N, N, 7, color::BLACK);
        rbtree_node_assert(&kvec[6].0, 8, N, 7, color::BLACK);
        rbtree_node_assert(&kvec[7].0, 5, 6, 4, color::RED);
        rbtree_node_assert(&kvec[8].0, N, N, 6, color::RED);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 4);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &3), 8);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &3), 3);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 7);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &2), 5);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &2), 7);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &1), 5);
        assert_eq!(kvec_rbtree!(kvec).find(root, &1), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 3);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), N);

        // +--------------------------+      +-----------------------+
        // |         4b:3             |      |         4b:3          |
        // |      /       \           |      |      /       \        |
        // |    7r:2     1r:3         |      |    7r:2     1r:3      |
        // |    /  \     /  \         |      |    /  \     /  \      |
        // | 5b:2 6b:3 0b:3 2b:3      | ---> | 5b:2 6b:3 0b:3 3b:3   |
        // |       /          \       |      |       /        / \    |
        // |    8r:3          3r:3    |      |    8r:3     2r:3 9r:4 |
        // |                    \     |      +-----------------------+
        // |                    9r:4* |
        // +--------------------------+
        kvec.push((RBTreeNode::new(), 4));
        kvec_rbtree_mut!(kvec).insert_last(&mut root, 9);
        rbtree_node_assert(&kvec[0].0, N, N, 1, color::BLACK);
        rbtree_node_assert(&kvec[1].0, 0, 3, 4, color::RED);
        rbtree_node_assert(&kvec[2].0, N, N, 3, color::RED);
        rbtree_node_assert(&kvec[3].0, 2, 9, 1, color::BLACK);
        rbtree_node_assert(&kvec[4].0, 7, 1, 4, color::BLACK);
        rbtree_node_assert(&kvec[5].0, N, N, 7, color::BLACK);
        rbtree_node_assert(&kvec[6].0, 8, N, 7, color::BLACK);
        rbtree_node_assert(&kvec[7].0, 5, 6, 4, color::RED);
        rbtree_node_assert(&kvec[8].0, N, N, 6, color::RED);
        rbtree_node_assert(&kvec[9].0, N, N, 3, color::RED);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 4);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &3), 8);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &3), 3);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 7);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &2), 5);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &2), 7);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &1), 5);
        assert_eq!(kvec_rbtree!(kvec).find(root, &1), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 9);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), 9);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &5), 9);
        assert_eq!(kvec_rbtree!(kvec).find(root, &5), N);

        // +-----------------------+      +-----------------------+      +-----------------------+      +-----------------------+
        // |         4b:3          |      |         4b:3          |      |         4r:3*         |      |         4b:3          |
        // |      /       \        |      |      /       \        |      |      /       \        |      |      /       \        |
        // |    7r:2     1r:3      |      |    7r:2     1r:3      |      |    7b:2     1b:3      |      |    7b:2     1b:3      |
        // |    /  \     /  \      |      |    /  \     /  \      |      |    /  \     /  \      |      |    /  \     /  \      |
        // | 5b:2 6b:3 0b:3 3b:3   | ---> | 5b:2 6b:3 0b:3 3r:3*  | ---> | 5b:2 6b:3 0b:3 3r:3   | ---> | 5b:2 6b:3 0b:3 3r:3   |
        // |       /        / \    |      |       /        / \    |      |       /        / \    |      |       /        / \    |
        // |    8r:3     2r:3 9r:4 |      |    8r:3     2b:3 9b:4 |      |    8r:3     2b:3 9b:4 |      |    8r:3     2b:3 9b:4 |
        // |                  /    |      |                  /    |      |                  /    |      |                  /    |
        // |               10r:3*  |      |               10r:3   |      |               10r:3   |      |               10r:3   |
        // +-----------------------+      +-----------------------+      +-----------------------+      +-----------------------+
        kvec.push((RBTreeNode::new(), 3));
        kvec_rbtree_mut!(kvec).insert_last(&mut root, 10);
        rbtree_node_assert(&kvec[0].0, N, N, 1, color::BLACK);
        rbtree_node_assert(&kvec[1].0, 0, 3, 4, color::BLACK);
        rbtree_node_assert(&kvec[2].0, N, N, 3, color::BLACK);
        rbtree_node_assert(&kvec[3].0, 2, 9, 1, color::RED);
        rbtree_node_assert(&kvec[4].0, 7, 1, 4, color::BLACK);
        rbtree_node_assert(&kvec[5].0, N, N, 7, color::BLACK);
        rbtree_node_assert(&kvec[6].0, 8, N, 7, color::BLACK);
        rbtree_node_assert(&kvec[7].0, 5, 6, 4, color::BLACK);
        rbtree_node_assert(&kvec[8].0, N, N, 6, color::RED);
        rbtree_node_assert(&kvec[9].0, 10, N, 3, color::BLACK);
        rbtree_node_assert(&kvec[10].0, N, N, 9, color::RED);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &3), 4);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &3), 8);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &3), 10);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &2), 7);
        assert_eq!(kvec_rbtree!(kvec).find_first(root, &2), 5);
        assert_eq!(kvec_rbtree!(kvec).find_last(root, &2), 7);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &1), 5);
        assert_eq!(kvec_rbtree!(kvec).find(root, &1), N);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &4), 9);
        assert_eq!(kvec_rbtree!(kvec).find(root, &4), 9);
        assert_eq!(kvec_rbtree!(kvec).locate(root, &5), 9);
        assert_eq!(kvec_rbtree!(kvec).find(root, &5), N);
    }
}
