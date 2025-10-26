use std::collections::HashMap;

use embed_struct::list::{ListMut, ListNode, ListNodeIndex};
use embed_struct::ptr::NullablePtr;

struct LRUCacheItem {
    key: String,
    value: String,
    node: ListNode,
}

impl LRUCacheItem {
    fn new() -> LRUCacheItem {
        LRUCacheItem {
            key: String::new(),
            value: String::new(),
            node: ListNode::new(),
        }
    }
}

struct ListCacheVec();

impl ListNodeIndex for ListCacheVec {
    type Container = Vec<LRUCacheItem>;

    fn index(c: &Self::Container, i: usize) -> &ListNode<usize> {
        &c[i].node
    }

    fn index_mut(c: &mut Self::Container, i: usize) -> &mut ListNode<usize> {
        &mut c[i].node
    }
}

pub struct LRUCache {
    cache: Vec<LRUCacheItem>,
    map: HashMap<String, usize>,
    queue_head: usize,
    free_head: usize,
}

macro_rules! cache_list_mut {
    ($p: expr) => {{ ListMut::<_, ListCacheVec>::new((&mut ($p).cache)) }};
}

impl LRUCache {
    pub fn new(capacity: usize) -> Self {
        let mut result = Self {
            cache: Vec::new(),
            map: HashMap::new(),
            queue_head: usize::nullptr(),
            free_head: usize::nullptr(),
        };
        for _ in 0..capacity {
            result.cache.push(LRUCacheItem::new());
        }
        for i in 0..capacity {
            cache_list_mut!(result).append(&mut result.free_head, i);
        }
        result
    }

    fn evict_oldest(&mut self) -> Option<()> {
        let tail = cache_list_mut!(self).pop_back(&mut self.queue_head);
        let tail = tail.to_option()?;
        self.map.remove(self.cache[tail].key.as_str());
        self.cache[tail] = LRUCacheItem::new();
        cache_list_mut!(self).append(&mut self.free_head, tail);
        Some(())
    }

    pub fn set(&mut self, key: &str, value: &str) {
        let slot = if let Some(val) = self.map.get(key.into()) {
            let slot = *val;
            assert!(self.cache[slot].node.in_list());
            cache_list_mut!(self).remove(&mut self.queue_head, slot);
            slot
        } else {
            if self.free_head.null() {
                self.evict_oldest();
            }
            cache_list_mut!(self).pop_front(&mut self.free_head)
        };
        assert!(!self.cache[slot].node.in_list());
        self.map.insert(key.into(), slot);
        self.cache[slot].key = key.into();
        self.cache[slot].value = value.into();
        cache_list_mut!(self).prepend(&mut self.queue_head, slot);
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        let slot = *self.map.get(key)?;
        cache_list_mut!(self).remove(&mut self.queue_head, slot);
        cache_list_mut!(self).prepend(&mut self.queue_head, slot);
        Some(self.cache[slot].value.as_str())
    }

    pub fn get_untouched(&self, key: &str) -> Option<&str> {
        let slot = *self.map.get(key)?;
        Some(self.cache[slot].value.as_str())
    }
}

#[cfg(test)]
mod test {
    use crate::LRUCache;

    #[test]
    fn test_normal() {
        let mut cache = LRUCache::new(8);
        assert_eq!(cache.map.len(), 0);
        cache.set("1", "a0");
        assert_eq!(cache.map.len(), 1);
        assert_eq!(cache.get("1").unwrap(), "a0");
        cache.set("1", "a1");
        assert_eq!(cache.map.len(), 1);
        assert_eq!(cache.get("1").unwrap(), "a1");
        cache.set("2", "b0");
        assert_eq!(cache.map.len(), 2);
        cache.set("3", "c0");
        assert_eq!(cache.map.len(), 3);
        cache.set("4", "d0");
        assert_eq!(cache.map.len(), 4);
        cache.set("5", "e0");
        assert_eq!(cache.map.len(), 5);
        cache.set("6", "f0");
        assert_eq!(cache.map.len(), 6);
        cache.set("7", "g0");
        assert_eq!(cache.map.len(), 7);
        cache.set("8", "h0");
        assert_eq!(cache.get_untouched("1").unwrap(), "a1");
        assert_eq!(cache.map.len(), 8);
        cache.set("9", "i0");
        assert_eq!(cache.get_untouched("1"), None);
        assert_eq!(cache.map.len(), 8);
    }
}
