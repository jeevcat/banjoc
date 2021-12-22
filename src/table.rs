use std::{cmp::max, iter};

use crate::{
    gc::{GarbageCollect, Gc, GcRef, MakeObj},
    obj::LoxString,
    value::Value,
};

struct Entry {
    // The table doesn't own any of the strings used as keys. Their lifetime is the responsibility of the gc
    key: Option<GcRef<LoxString>>,
    value: Value,
}

pub struct Table {
    // Number of populated entries plus tombstones
    count: usize,
    entries: Vec<Entry>,
}

impl Table {
    const MAX_LOAD: f64 = 0.75;
    pub fn new() -> Self {
        Self {
            count: 0,
            entries: vec![],
        }
    }

    pub fn insert(&mut self, key: GcRef<LoxString>, value: Value) -> bool {
        dbg!(key);
        dbg!(value);
        if self.count + 1 > (self.capacity() as f64 * Table::MAX_LOAD) as usize {
            self.grow();
        }

        let entry = find_entry_mut(&mut self.entries, key);
        let is_new_key = entry.key.is_none();
        if is_new_key && matches!(entry.value, Value::Nil) {
            self.count += 1;
        }
        entry.key = Some(key);
        entry.value = value;

        dbg!(self.count);

        is_new_key
    }

    pub fn get(&self, key: GcRef<LoxString>) -> Option<Value> {
        if self.count == 0 {
            return None;
        }

        let entry = find_entry(&self.entries, key);
        if entry.key.is_some() {
            Some(entry.value)
        } else {
            None
        }
    }

    pub fn remove(&mut self, key: GcRef<LoxString>) -> bool {
        println!("Remove {:?}", key);
        if self.count == 0 {
            return false;
        }

        let entry = find_entry_mut(&mut self.entries, key);
        if entry.key.is_none() {
            return false;
        }

        // Place a tombstone in the entry
        entry.key = None;
        entry.value = Value::Bool(true);
        true
    }

    pub fn find_string(&self, string: &str, hash: u32) -> Option<GcRef<LoxString>> {
        if self.count == 0 {
            return None;
        }

        let mut index = hash as usize % self.entries.len();

        loop {
            let entry = &self.entries[index];
            match entry.key {
                Some(key) => {
                    if key.as_str().len() == string.len()
                        && key.hash == hash
                        && key.as_str() == string
                    {
                        // We found it
                        return Some(key);
                    }
                }
                None => {
                    // Stop if we find an empty non-tombstone entry
                    if matches!(entry.value, Value::Nil) {
                        return None;
                    }
                }
            }
            index = (index + 1) % self.capacity();
        }
    }

    pub fn remove_white(&mut self) {
        for i in 0..self.capacity() {
            let entry = &self.entries[i];
            if let Some(key) = entry.key {
                if !key.is_marked() {
                    self.remove(key);
                }
            }
        }
    }

    fn grow(&mut self) {
        // Double the capacity
        let new_capacity = max(8, self.capacity() * 2);
        let mut new: Vec<_> = iter::repeat_with(|| Entry {
            key: None,
            value: Value::Nil,
        })
        .take(new_capacity)
        .collect();

        self.count = 0;
        for entry in &self.entries {
            if let Some(key) = entry.key {
                let dest = find_entry_mut(&mut new, key);
                dest.key = entry.key;
                dest.value = entry.value;
                self.count += 1;
            }
        }

        self.entries = new;
    }

    fn capacity(&self) -> usize {
        self.entries.len()
    }
}

fn find_entry(entries: &[Entry], key: GcRef<LoxString>) -> &Entry {
    let mut index = key.hash as usize % entries.len();
    // The first seen tombstone
    let mut tombstone = None;

    loop {
        let entry = &entries[index];
        if let Some(k) = entry.key {
            if k == key {
                // We found the key
                return entry;
            }
        } else {
            match entry.value {
                Value::Nil => {
                    // Empty entry
                    return if let Some(tombstone) = tombstone {
                        tombstone
                    } else {
                        entry
                    };
                }
                _ => {
                    // We found a tombstone
                    if tombstone.is_none() {
                        tombstone = Some(entry);
                    }
                }
            }
        }

        // Collision: linear probe
        index = (index + 1) % entries.len();
    }
}

fn find_entry_mut(entries: &mut [Entry], key: GcRef<LoxString>) -> &mut Entry {
    let len = entries.len();
    let mut index = key.hash as usize % len;
    // The first seen tombstone
    let mut tombstone = None;

    loop {
        let entry = &entries[index];
        if let Some(k) = entry.key {
            if k == key {
                // We found the key
                return &mut entries[index];
            }
        } else {
            match entry.value {
                Value::Nil => {
                    // Empty entry
                    return if let Some(tombstone) = tombstone {
                        &mut entries[tombstone]
                    } else {
                        &mut entries[index]
                    };
                }
                _ => {
                    // We found a tombstone
                    if tombstone.is_none() {
                        tombstone = Some(index);
                    }
                }
            }
        }

        // Collision: linear probe
        index = (index + 1) % len;
    }
}

impl GarbageCollect for Table {
    fn mark(&mut self, gc: &mut Gc) {
        for entry in &mut self.entries {
            if let Some(mut key) = entry.key {
                key.mark(gc);
                entry.value.mark(gc)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use super::*;

    #[test]
    fn insertion() {
        // Generate some strings
        let mut strings: Vec<_> = (0..100000).map(|n| LoxString::new(n.to_string())).collect();

        // Simulate being held by gc
        let refs = make_refs(&mut strings);

        // Insert into Table
        let mut t = Table::new();
        let mut count = 0;
        for key in &refs {
            let num = str_to_num(*key) as f64;
            t.insert(*key, Value::Number(num));
            count += 1;
            assert_eq!(t.count, count);
        }

        // Check inserted values
        for key in refs {
            let num = to_num(t.get(key).unwrap());
            assert_eq!(str_to_num(key), num);
        }
    }

    #[test]
    fn deletion() {
        // Generate some strings
        let mut strings: Vec<_> = (0..1000).map(|n| LoxString::new(n.to_string())).collect();

        // Simulate being held by gc
        let mut refs = make_refs(&mut strings);

        // Insert into Table
        let mut t = Table::new();
        for key in &refs {
            let num = str_to_num(*key) as f64;
            t.insert(*key, Value::Number(num));
        }

        // Remove every third entry
        for i in 0..250 {
            let index = i * 3;
            t.remove(refs[index]);
            refs.remove(index);
        }

        // Check inserted values
        for key in refs {
            let num = to_num(t.get(key).unwrap());
            assert_eq!(str_to_num(key), num);
        }
    }

    fn make_refs(strings: &mut [LoxString]) -> Vec<GcRef<LoxString>> {
        strings
            .iter_mut()
            .map(|ls| GcRef {
                pointer: unsafe { NonNull::new_unchecked(ls as *mut _) },
            })
            .collect()
    }

    fn to_num(value: Value) -> i32 {
        if let Value::Number(num) = value {
            num as i32
        } else {
            unreachable!()
        }
    }

    fn str_to_num(string: GcRef<LoxString>) -> i32 {
        string.as_str().parse().unwrap()
    }
}
