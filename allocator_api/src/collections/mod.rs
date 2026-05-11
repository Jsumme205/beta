mod map;
mod raw_vec;

use core::{
    borrow::Borrow,
    hash::{BuildHasher, Hash},
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ops::{self, RangeBounds},
    ptr::{self, NonNull},
    slice,
};

use map::{Bucket, RawTable, SizedTypeProps};

use crate::{Allocator, collections::map::Properties};

pub trait Equivalent<K: ?Sized> {
    fn equiv(&self, key: &K) -> bool;
}

impl<Q: ?Sized, K: ?Sized> Equivalent<K> for Q
where
    Q: Eq,
    K: core::borrow::Borrow<Q>,
{
    fn equiv(&self, key: &K) -> bool {
        self == key.borrow()
    }
}

pub(crate) fn equiv_key<Q, K>(k: &Q) -> impl Fn(*const ()) -> bool + '_
where
    Q: Equivalent<K> + ?Sized,
{
    move |x| k.equiv(unsafe { &*(x as *const K) })
}

pub(crate) fn make_hasher<Q, V, S>(hash_builder: &S) -> impl Fn(*const ()) -> u64 + use<'_, Q, V, S>
where
    Q: Hash,
    S: BuildHasher,
{
    move |val| make_hash::<Q, S>(hash_builder, unsafe { &(&*(val as *const (Q, V))).0 })
}

pub(crate) fn make_hash<Q, S>(hash_builder: &S, val: &Q) -> u64
where
    Q: Hash + ?Sized,
    S: BuildHasher,
{
    hash_builder.hash_one(val)
}

pub struct HashMap<K, V, S, A> {
    hash_builder: S,
    table: RawTable<A>,
    _marker: PhantomData<(K, V)>,
}

impl<K, V, S, A> HashMap<K, V, S, A> {
    const ELEM_PROPERTIES: Properties = <(K, V) as SizedTypeProps>::PROPERTIES;

    pub const fn with_hasher(hash_builder: S) -> Self {
        Self {
            hash_builder,
            table: RawTable::new(),
            _marker: PhantomData,
        }
    }

    pub const fn len(&self) -> usize {
        self.table.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

impl<K, V, S, A> HashMap<K, V, S, A>
where
    A: Allocator,
{
    /// # Safety
    ///
    pub unsafe fn deinit(&mut self, alloc: &A) {
        unsafe {
            self.table.deinit(alloc, Self::ELEM_PROPERTIES);
        }
    }
}

impl<K, V, S, A> HashMap<K, V, S, A>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        todo!()
    }
}

impl<K, V, S, A> HashMap<K, V, S, A>
where
    K: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    pub fn reserve(&mut self, additional: usize, alloc: &A) {
        unsafe {
            self.table.reserve(
                additional,
                &make_hasher::<K, V, S>(&self.hash_builder),
                Self::ELEM_PROPERTIES,
                alloc,
            )
        };
    }

    pub fn insert(&mut self, key: K, value: V, alloc: &A) -> Option<V> {
        let hash = make_hash::<K, S>(&self.hash_builder, &key);
        match self.find_or_find_insert_index(hash, &key, alloc) {
            Ok(mut bucket) => Some(mem::replace(unsafe { &mut bucket.as_mut().1 }, value)),
            Err(index) => unsafe {
                self.table.insert_at_index(hash, index, (key, value));
                None
            },
        }
    }

    fn find_or_find_insert_index<Q>(
        &mut self,
        hash: u64,
        key: &Q,
        alloc: &A,
    ) -> Result<Bucket<(K, V)>, usize>
    where
        Q: Equivalent<K>,
    {
        unsafe {
            match self.table.find_or_find_insert_index(
                hash,
                &mut equiv_key(key),
                &make_hasher::<K, V, S>(&self.hash_builder),
                alloc,
                Self::ELEM_PROPERTIES,
            ) {
                Ok(bucket) => Ok(Bucket::from_opaque(bucket)),
                Err(e) => Err(e),
            }
        }
    }

    pub fn entry(&mut self, key: K, alloc: &A) -> Entry<'_, K, V, A> {
        let hash = make_hash(&self.hash_builder, &key);

        fn __find<K, V>(ptr: *const (), key: &K) -> bool
        where
            K: Eq + Hash,
        {
            let r = unsafe { &*(ptr as *const (K, V)) };
            r.0.eq(key)
        }

        unsafe {
            if let Some(elem) = self.table.find(
                hash,
                &mut |q| __find::<_, V>(q, &key),
                Self::ELEM_PROPERTIES,
            ) {
                Entry::Occupied(OccupiedEntry {
                    elem: Bucket::from_opaque(elem),
                    _table: &mut self.table,
                })
            } else {
                self.reserve(1, alloc);

                Entry::Vacant(VacantEntry {
                    hash,
                    key,
                    table: &mut self.table,
                    _marker: PhantomData,
                })
            }
        }
    }
}

pub enum Entry<'a, K, V, A> {
    Occupied(OccupiedEntry<'a, K, V, A>),
    Vacant(VacantEntry<'a, K, V, A>),
}

impl<'a, K, V, A: Allocator> Entry<'a, K, V, A>
where
    K: 'a,
    V: 'a,
{
    pub fn or_insert(self, default: V) -> &'a mut V
    where
        K: Hash,
    {
        match self {
            Self::Occupied(entry) => entry.into_mut(),
            Self::Vacant(vacant) => vacant.insert(default),
        }
    }
}

pub struct OccupiedEntry<'a, K, V, A> {
    elem: Bucket<(K, V)>,
    _table: &'a mut RawTable<A>,
}

impl<'a, K, V, A> OccupiedEntry<'a, K, V, A>
where
    K: 'a,
    V: 'a,
{
    pub fn into_mut(self) -> &'a mut V {
        unsafe { &mut self.elem.into_mut().1 }
    }
}

pub struct VacantEntry<'a, K, V, A> {
    hash: u64,
    key: K,
    table: &'a mut RawTable<A>,
    _marker: PhantomData<Bucket<(K, V)>>,
}

impl<'a, K, V, A> VacantEntry<'a, K, V, A>
where
    A: Allocator,
    K: 'a,
    V: 'a,
{
    pub fn insert(self, value: V) -> &'a mut V {
        unsafe {
            let bucket = self.table.insert_no_grow(self.hash, (self.key, value));
            &mut bucket.into_mut().1
        }
    }
}

pub struct Vec<T, A> {
    raw: raw_vec::RawVecInner,
    len: usize,
    _marker: PhantomData<(T, A)>,
}

impl<T, A> Vec<T, A> {
    pub const fn new() -> Self {
        todo!()
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub const fn as_slice(&self) -> &[T] {
        todo!()
    }

    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        todo!()
    }

    pub fn get<Idx>(&self, index: Idx) -> Option<&<Idx as VecIndex<T, A>>::Output>
    where
        Idx: VecIndex<T, A>,
    {
        index.index(self)
    }

    pub fn get_mut<Idx>(&mut self, index: Idx) -> Option<&mut <Idx as VecIndex<T, A>>::Output>
    where
        Idx: VecIndex<T, A>,
    {
        index.index_mut(self)
    }
}

impl<T, A> Vec<Option<T>, A>
where
    A: Allocator,
{
    pub fn allocate(&mut self, alloc: &A) -> usize {
        let len = self.len;
        self.push(None, alloc);
        len
    }
}

impl<T, A> Vec<T, A>
where
    A: Allocator,
{
    pub fn push(&mut self, item: T, alloc: &A) {
        unsafe {
            let item = MaybeUninit::new(item);

            self.raw
                .push(T::PROPERTIES, alloc, &mut self.len, &mut |slot| {
                    ptr::write(slot as *mut T, item.assume_init_read());
                });
        }
    }

    unsafe fn __deinit(&mut self, alloc: &A) {}

    pub fn drain<R>(&mut self, range: R) -> Drain<'_, T, A>
    where
        R: RangeBounds<usize>,
    {
        todo!()
    }
}

unsafe impl<T, A: Allocator> crate::Deinit<A> for Vec<T, A> {
    unsafe fn deinit(&mut self, allocator: &A) {
        unsafe {
            self.__deinit(allocator);
        }
    }
}

impl<T, A, I> core::ops::Index<I> for Vec<T, A>
where
    I: VecIndex<T, A>,
{
    type Output = <I as VecIndex<T, A>>::Output;

    fn index(&self, index: I) -> &Self::Output {
        index.index(self).expect("out of bounds")
    }
}

impl<T, A, I> core::ops::IndexMut<I> for Vec<T, A>
where
    I: VecIndex<T, A>,
{
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        index.index_mut(self).unwrap()
    }
}

pub unsafe trait VecIndex<T, A> {
    type Output: ?Sized;

    fn index(self, vec: &Vec<T, A>) -> Option<&Self::Output>;

    fn index_mut(self, vec: &mut Vec<T, A>) -> Option<&mut Self::Output>;

    unsafe fn index_unchecked(self, vec: &Vec<T, A>) -> *const Self::Output;

    unsafe fn index_unchecked_mut(self, vec: &mut Vec<T, A>) -> *mut Self::Output;
}

unsafe impl<T, A, I> VecIndex<T, A> for I
where
    I: core::slice::SliceIndex<[T]> + Clone,
{
    type Output = <I as core::slice::SliceIndex<[T]>>::Output;

    fn index(self, vec: &Vec<T, A>) -> Option<&Self::Output> {
        vec.as_slice().get(self)
    }

    fn index_mut(self, vec: &mut Vec<T, A>) -> Option<&mut Self::Output> {
        vec.as_mut_slice().get_mut(self)
    }

    unsafe fn index_unchecked(self, vec: &Vec<T, A>) -> *const Self::Output {
        unsafe { vec.as_slice().get_unchecked(self) }
    }

    unsafe fn index_unchecked_mut(self, vec: &mut Vec<T, A>) -> *mut Self::Output {
        unsafe { vec.as_mut_slice().get_unchecked_mut(self) }
    }
}

pub struct Drain<'a, T, A> {
    tail_start: usize,
    tail_len: usize,
    iter: slice::Iter<'a, T>,
    vec: NonNull<Vec<T, A>>,
}

fn range<R>(range: R, bounds: ops::RangeTo<usize>) -> ops::Range<usize>
where
    R: ops::RangeBounds<usize>,
{
    let len = bounds.end;
    todo!()
}

impl<T, A> Iterator for Drain<'_, T, A> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}
