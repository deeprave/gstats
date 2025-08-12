//! Priority Queue Implementation for Plugin Registration
//!
//! Provides a Vec-based priority queue with priority-based insertion and efficient removal.

#[derive(Debug)]
pub struct PriorityQueue<T> {
    items: Vec<(i32, T)>, // (priority, item)
}

impl<T> PriorityQueue<T> {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }
    
    pub fn push(&mut self, priority: i32, item: T) {
        // Find insertion point to maintain order (high to low priority)
        let pos = self.items
            .binary_search_by(|(p, _)| priority.cmp(p))
            .unwrap_or_else(|e| e);
        self.items.insert(pos, (priority, item));
    }
    
    pub fn pop(&mut self) -> Option<(i32, T)> {
        if self.items.is_empty() { 
            None 
        } else { 
            Some(self.items.remove(0)) 
        }
    }
    
    // Remove by predicate
    pub fn remove_by<F>(&mut self, predicate: F) -> Option<(i32, T)>
    where
        F: Fn(&T) -> bool,
    {
        if let Some(pos) = self.items.iter().position(|(_, item)| predicate(item)) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }
    
    // Remove by index (if you know the position)
    pub fn remove_at(&mut self, index: usize) -> Option<(i32, T)> {
        if index < self.items.len() {
            Some(self.items.remove(index))
        } else {
            None
        }
    }
    
    // Immutable iteration
    pub fn iter(&self) -> impl Iterator<Item = &(i32, T)> {
        self.items.iter()
    }
    
    // Mutable iteration
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut (i32, T)> {
        self.items.iter_mut()
    }
    
    // Get mutable reference to item by predicate
    pub fn get_mut<F>(&mut self, predicate: F) -> Option<&mut T>
    where
        F: Fn(&T) -> bool,
    {
        self.items.iter_mut()
            .find(|(_, item)| predicate(item))
            .map(|(_, item)| item)
    }
    
    // Find index by predicate (useful for later removal)
    pub fn find_index<F>(&self, predicate: F) -> Option<usize>
    where
        F: Fn(&T) -> bool,
    {
        self.items.iter().position(|(_, item)| predicate(item))
    }
    
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    
    pub fn len(&self) -> usize {
        self.items.len()
    }
}

// If T implements PartialEq, we can remove by value
impl<T: PartialEq> PriorityQueue<T> {
    pub fn remove(&mut self, item: &T) -> Option<(i32, T)> {
        self.remove_by(|x| x == item)
    }
}

impl<T> Default for PriorityQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_queue_ordering() {
        let mut pq = PriorityQueue::new();
        
        // Add items with different priorities
        pq.push(5, "medium");
        pq.push(10, "high");
        pq.push(1, "low");
        pq.push(10, "also_high");
        
        // Should pop in priority order (high to low)
        // For equal priorities, insertion order is preserved
        let first = pq.pop();
        assert!(matches!(first, Some((10, _)))); // Should be priority 10
        
        let second = pq.pop();
        assert!(matches!(second, Some((10, _)))); // Should be priority 10
        
        assert_eq!(pq.pop(), Some((5, "medium")));
        assert_eq!(pq.pop(), Some((1, "low")));
        assert_eq!(pq.pop(), None);
    }

    #[test]
    fn test_priority_queue_remove_by_predicate() {
        let mut pq = PriorityQueue::new();
        
        pq.push(5, "item1".to_string());
        pq.push(10, "item2".to_string());
        pq.push(1, "item3".to_string());
        
        // Remove by predicate
        let removed = pq.remove_by(|item| item == "item2");
        assert_eq!(removed, Some((10, "item2".to_string())));
        assert_eq!(pq.len(), 2);
        
        // Verify order is maintained
        assert_eq!(pq.pop(), Some((5, "item1".to_string())));
        assert_eq!(pq.pop(), Some((1, "item3".to_string())));
    }

    #[test]
    fn test_priority_queue_iteration() {
        let mut pq = PriorityQueue::new();
        
        pq.push(5, "medium");
        pq.push(10, "high");
        pq.push(1, "low");
        
        // Test immutable iteration
        let priorities: Vec<i32> = pq.iter().map(|(p, _)| *p).collect();
        assert_eq!(priorities, vec![10, 5, 1]);
        
        // Test find index
        let index = pq.find_index(|item| *item == "medium");
        assert_eq!(index, Some(1));
    }
}