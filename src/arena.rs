use std::marker::PhantomData;

#[derive(Debug)]
pub struct Arena<T> {
    elements: Vec<T>,
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self {
            elements: Vec::default(),
        }
    }

    pub fn allocate(&mut self, t: T) -> Handle<T> {
        let id = self.elements.len() as u32;
        self.elements.push(t);
        Handle {
            id,
            _ghost: PhantomData,
        }
    }

    pub fn get(&self, handle: Handle<T>) -> &T {
        self.elements.get(handle.id as usize).expect("bad handle")
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        self.elements
            .get_mut(handle.id as usize)
            .expect("bad handle")
    }

    pub fn replace(&mut self, handle: Handle<T>, t: T) -> T {
        let dst = self.get_mut(handle);
        std::mem::replace(dst, t)
    }

    pub fn elements(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.elements
            .iter()
            .enumerate()
            .map(|(i, el)| (Handle::new(i as u32), el))
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Handle<T> {
    id: u32,
    _ghost: PhantomData<*const T>,
}

impl<T> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Handle").field("id", &self.id).finish()
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _ghost: self._ghost,
        }
    }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}
impl<T> Eq for Handle<T> {}
impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl<T> Handle<T> {
    fn new(id: u32) -> Self {
        Handle {
            id,
            _ghost: PhantomData,
        }
    }
}
