use std::{
    any::{Any, TypeId},
    marker::PhantomData,
};

#[derive(Debug, Clone)]
pub struct Arena<T> {
    slots: Vec<T>,
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Self {
            slots: Vec::default(),
        }
    }

    pub fn allocate(&mut self, t: T) -> Handle<T> {
        let id = self.slots.len() as u32;
        self.slots.push(t);
        Handle {
            id,
            _ghost: PhantomData,
        }
    }

    pub fn get(&self, handle: Handle<T>) -> &T {
        self.slots.get(handle.id as usize).expect("bad handle")
    }

    pub fn get_mut(&mut self, handle: Handle<T>) -> &mut T {
        self.slots.get_mut(handle.id as usize).expect("bad handle")
    }

    pub fn replace(&mut self, handle: Handle<T>, t: T) -> T {
        let dst = self.get_mut(handle);
        std::mem::replace(dst, t)
    }

    pub fn elements(&self) -> impl Iterator<Item = (Handle<T>, &T)> {
        self.slots
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

impl<T> Handle<T> {
    fn new(id: u32) -> Self {
        Handle {
            id,
            _ghost: PhantomData,
        }
    }
}

impl<T: Any> Handle<T> {
    pub fn to_type_erased(self) -> TypeErasedHandle {
        TypeErasedHandle {
            id: self.id,
            erased_type_id: TypeId::of::<T>(),
        }
    }

    pub unsafe fn transmute<U: Any>(self) -> Handle<U> {
        Handle {
            id: self.id,
            _ghost: PhantomData,
        }
    }
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
impl<T> PartialOrd for Handle<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}
impl<T> Ord for Handle<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}
impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}
unsafe impl<T> Send for Handle<T> {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeErasedHandle {
    id: u32,
    erased_type_id: TypeId,
}

impl TypeErasedHandle {
    pub fn downcast<T: Any>(self) -> Result<Handle<T>, Self> {
        if self.erased_type_id == TypeId::of::<T>() {
            Ok(Handle::new(self.id))
        } else {
            Err(self)
        }
    }

    pub fn erased_type_id(&self) -> TypeId {
        self.erased_type_id
    }

    pub unsafe fn transmute<T: Any>(self) -> Handle<T> {
        Handle {
            id: self.id,
            _ghost: PhantomData,
        }
    }
}

impl<T: Any> PartialEq<Handle<T>> for TypeErasedHandle {
    fn eq(&self, other: &Handle<T>) -> bool {
        *self == other.to_type_erased()
    }
}
