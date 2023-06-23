use std::{
    any::{Any, TypeId},
    marker::PhantomData,
};

#[derive(Debug, Clone)]
pub struct Arena<T, H = Handle<T>> {
    elements: Vec<T>,
    _ghost: PhantomData<*const H>,
}

impl<T, U> Arena<T, Handle<U>> {
    pub fn new() -> Self {
        Self {
            elements: Vec::default(),
            _ghost: PhantomData,
        }
    }

    pub fn allocate(&mut self, t: T) -> Handle<U> {
        let id = self.elements.len() as u32;
        self.elements.push(t);
        Handle {
            id,
            _ghost: PhantomData,
        }
    }

    pub fn get(&self, handle: Handle<U>) -> &T {
        self.elements.get(handle.id as usize).expect("bad handle")
    }

    pub fn get_mut(&mut self, handle: Handle<U>) -> &mut T {
        self.elements
            .get_mut(handle.id as usize)
            .expect("bad handle")
    }

    pub fn replace(&mut self, handle: Handle<U>, t: T) -> T {
        let dst = self.get_mut(handle);
        std::mem::replace(dst, t)
    }

    pub fn elements(&self) -> impl Iterator<Item = (Handle<U>, &T)> {
        self.elements
            .iter()
            .enumerate()
            .map(|(i, el)| (Handle::new(i as u32), el))
    }
}

impl<T, U> Default for Arena<T, Handle<U>> {
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

impl<T: Any> Handle<T> {
    pub fn to_untyped(self) -> UntypedHandle {
        UntypedHandle {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UntypedHandle {
    id: u32,
    erased_type_id: TypeId,
}

impl UntypedHandle {
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

impl<T: Any> PartialEq<Handle<T>> for UntypedHandle {
    fn eq(&self, other: &Handle<T>) -> bool {
        *self == other.to_untyped()
    }
}
