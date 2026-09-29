//! Per-entity view state keyed by entity id (the TypeScript views' `Map<Entity, State>`).
//!
//! Entry order follows the ids, and ids only grow, so iteration matches the order the
//! TypeScript `Map`s saw entities in. Lookups are binary searches; new entities append.

pub struct IdMap<V> {
    entries: Vec<(u32, V)>,
}

impl<V> Default for IdMap<V> {
    fn default() -> Self {
        IdMap { entries: Vec::new() }
    }
}

impl<V> IdMap<V> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    fn position(&self, id: u32) -> Result<usize, usize> {
        match self.entries.last() {
            // Fast path: the newest entity.
            Some(&(last, _)) if last < id => Err(self.entries.len()),
            _ => self.entries.binary_search_by_key(&id, |&(key, _)| key),
        }
    }

    pub fn get(&self, id: u32) -> Option<&V> {
        self.position(id).ok().map(|index| &self.entries[index].1)
    }

    /// The entry for `id`, created by `create` (given the current entry count) when missing.
    pub fn get_or_insert_with(&mut self, id: u32, create: impl FnOnce(usize) -> V) -> &mut V {
        let index = match self.position(id) {
            Ok(index) => index,
            Err(index) => {
                let value = create(self.entries.len());
                self.entries.insert(index, (id, value));
                index
            }
        };
        &mut self.entries[index].1
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &V)> {
        self.entries.iter().map(|(id, value)| (*id, value))
    }

    /// Keeps the entries for which `keep` returns true, in order.
    pub fn retain(&mut self, mut keep: impl FnMut(&mut V) -> bool) {
        self.entries.retain_mut(|(_, value)| keep(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_id_order_and_finds_entries() {
        let mut map = IdMap::default();
        for id in [3, 7, 5, 9] {
            *map.get_or_insert_with(id, |count| count as u32 * 10) += id;
        }
        assert_eq!(map.iter().map(|(id, _)| id).collect::<Vec<_>>(), [3, 5, 7, 9]);
        assert_eq!(map.get(5), Some(&(20 + 5)));
        assert_eq!(map.get(4), None);
        map.retain(|value| *value % 2 == 1);
        assert_eq!(map.len(), 4 - map.iter().filter(|(_, value)| *value % 2 == 0).count());
        map.clear();
        assert!(map.is_empty());
    }
}
