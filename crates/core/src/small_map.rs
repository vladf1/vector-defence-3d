//! A tiny insertion-ordered map over a `Vec`, for the few-entry maps of the simulation (drone
//! assignments, stored progress). It avoids `HashMap`, whose SipHash and table code would add
//! about 10 KB to the Wasm for maps that never hold more than a few dozen entries.
#[derive(Clone, Debug)]
pub struct SmallMap<K, V> {
    entries: Vec<(K, V)>,
}

impl<K, V> Default for SmallMap<K, V> {
    fn default() -> Self {
        SmallMap { entries: Vec::new() }
    }
}

impl<K: PartialEq, V> SmallMap<K, V> {
    pub fn new() -> Self {
        SmallMap { entries: Vec::new() }
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: PartialEq<Q>,
        Q: ?Sized,
    {
        self.entries.iter().find(|(candidate, _)| candidate == key).map(|(_, value)| value)
    }

    pub fn insert(&mut self, key: K, value: V) {
        match self.entries.iter_mut().find(|(candidate, _)| *candidate == key) {
            Some(entry) => entry.1 = value,
            None => self.entries.push((key, value)),
        }
    }

    pub fn remove<Q>(&mut self, key: &Q)
    where
        K: PartialEq<Q>,
        Q: ?Sized,
    {
        self.entries.retain(|(candidate, _)| candidate != key);
    }

    /// The value for `key`, inserting `default` first when it is missing.
    pub fn or_insert(&mut self, key: K, default: V) -> &mut V {
        let index = match self.entries.iter().position(|(candidate, _)| *candidate == key) {
            Some(index) => index,
            None => {
                self.entries.push((key, default));
                self.entries.len() - 1
            }
        };
        &mut self.entries[index].1
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
