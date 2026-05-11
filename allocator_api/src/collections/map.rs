use core::{
    alloc::{Layout, LayoutError},
    hint,
    marker::PhantomData,
    mem::{self, ManuallyDrop},
    ptr::{self, NonNull},
    slice,
};

use crate::{
    AllocError, Allocator,
    collections::map::{
        bitmask::BitMaskIter,
        group::Group,
        tag::{Tag, TagSliceExt},
    },
};

struct RawTableInner {
    data: NonNull<u8>,
    metadata: Metadata,
}

pub(super) struct RawTable<A> {
    inner: RawTableInner,
    _marker: PhantomData<A>,
}

impl<A> RawTable<A> {
    pub const fn new() -> Self {
        Self {
            inner: RawTableInner::NEW,
            _marker: PhantomData,
        }
    }
}

impl<A> RawTable<A>
where
    A: Allocator,
{
    pub unsafe fn reserve(
        &mut self,
        additional: usize,
        hasher: &dyn Fn(*const ()) -> u64,
        type_properties: Properties,
        alloc: &A,
    ) {
        if unlikely(additional > self.inner.metadata.growth_left) {
            unsafe {
                if self
                    .reserve_rehash(
                        additional,
                        hasher,
                        Fallibilty::Infallible,
                        type_properties,
                        alloc,
                        type_properties.drop_fn,
                    )
                    .is_err()
                {
                    hint::unreachable_unchecked()
                }
            }
        }
    }

    #[allow(unused)]
    pub unsafe fn insert<T>(
        &mut self,
        hash: u64,
        value: T,
        hasher: &dyn Fn(*const ()) -> u64,
        alloc: &A,
    ) -> Bucket<T> {
        unsafe {
            let mut index = self.inner.find_insert_index(hash);
            let old_ctrl = *self.inner.ctrl(index);

            if unlikely(self.inner.metadata.growth_left == 0 && old_ctrl.special_is_empty()) {
                self.reserve(1, hasher, T::PROPERTIES, alloc);
                index = self.inner.find_insert_index(hash);
            }

            self.insert_at_index(hash, index, value)
        }
    }

    pub unsafe fn insert_no_grow<T>(&mut self, hash: u64, value: T) -> Bucket<T> {
        let (index, old_ctrl) = unsafe { self.inner.prep_insert_index(hash) };
        let mut bucket =
            unsafe { Bucket::from_opaque(self.inner.bucket(index, T::PROPERTIES.layout)) };
        self.inner.metadata.growth_left -= old_ctrl.special_is_empty() as usize;
        unsafe { bucket.write(value) };
        self.inner.metadata.items += 1;
        bucket
    }

    pub unsafe fn find(
        &self,
        hash: u64,
        eq: &mut dyn FnMut(*const ()) -> bool,
        props: Properties,
    ) -> Option<OpaqueBucket> {
        unsafe {
            let result = self.inner.find_inner(hash, &mut |index| {
                eq(self.inner.bucket(index, props.layout).ptr.as_ptr())
            });

            match result {
                Some(index) => Some(self.inner.bucket(index, props.layout)),
                None => None,
            }
        }
    }

    pub unsafe fn insert_at_index<T>(&mut self, hash: u64, index: usize, value: T) -> Bucket<T> {
        unsafe { self.insert_tagged_at_index(Tag::full(hash), index, value) }
    }

    pub unsafe fn insert_tagged_at_index<T>(
        &mut self,
        tag: Tag,
        index: usize,
        value: T,
    ) -> Bucket<T> {
        unsafe {
            let old = *self.inner.ctrl(index);
            self.inner.record_item_insert_at(index, old, tag);
            let mut bucket = Bucket::from_opaque(self.inner.bucket(index, T::PROPERTIES.layout));
            bucket.write(value);
            bucket
        }
    }

    unsafe fn reserve_rehash(
        &mut self,
        additional: usize,
        hasher: &dyn Fn(*const ()) -> u64,
        fallibility: Fallibilty,
        elem_props: Properties,
        alloc: &A,
        drop: unsafe fn(*mut u8),
    ) -> Result<(), AllocError> {
        unsafe {
            self.inner.reserve_rehash_inner(
                alloc,
                additional,
                &|table, index| hasher(table.bucket(index, elem_props.layout).ptr.as_ptr()),
                fallibility,
                elem_props.table_layout,
                elem_props.layout,
                elem_props.layout.size(),
                if elem_props.needs_drop {
                    Some(drop)
                } else {
                    None
                },
            )
        }
    }

    pub(super) unsafe fn deinit(&mut self, alloc: &A, type_properties: Properties) {
        unsafe {
            self.inner.drop_inner(alloc, type_properties);
        }
    }

    pub(super) unsafe fn find_or_find_insert_index(
        &mut self,
        hash: u64,
        eq: &mut dyn FnMut(*const ()) -> bool,
        hasher: &dyn Fn(*const ()) -> u64,
        alloc: &A,
        type_properties: Properties,
    ) -> Result<OpaqueBucket, usize> {
        unsafe {
            self.reserve(1, hasher, type_properties, alloc);

            match self
                .inner
                .find_or_find_insert_index_inner(hash, &mut |index| {
                    eq(self
                        .inner
                        .bucket(index, type_properties.layout)
                        .ptr
                        .as_ptr())
                }) {
                Ok(index) => Ok(self.inner.bucket(index, type_properties.layout)),
                Err(index) => Err(index),
            }
        }
    }
}

impl<A> RawTable<A> {
    pub const fn len(&self) -> usize {
        self.inner.metadata.items
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub(super) struct OpaqueBucket {
    ptr: NonNull<()>,
}

pub(super) struct Bucket<T> {
    opaque: OpaqueBucket,
    _marker: PhantomData<T>,
}

impl<T> Bucket<T> {
    pub(super) unsafe fn from_opaque(opaque: OpaqueBucket) -> Self {
        Self {
            opaque,
            _marker: PhantomData,
        }
    }

    #[allow(unused)]
    unsafe fn from_base_index(base: NonNull<T>, index: usize) -> Self {
        unsafe {
            Self::from_opaque(OpaqueBucket::from_base_index(
                base.cast(),
                index,
                T::PROPERTIES.layout,
            ))
        }
    }

    const unsafe fn write(&mut self, val: T) {
        unsafe {
            self.opaque.ptr.cast::<T>().write(val);
        }
    }

    pub(super) const unsafe fn as_mut(&mut self) -> &mut T {
        unsafe { self.opaque.ptr.cast().as_mut() }
    }

    pub(super) const unsafe fn into_mut<'a>(self) -> &'a mut T {
        unsafe { self.opaque.ptr.cast().as_mut() }
    }
}

impl OpaqueBucket {
    unsafe fn from_base_index(base: NonNull<u8>, index: usize, elem_layout: Layout) -> Self {
        unsafe {
            let ptr = if elem_layout.size() == 0 {
                invalid_mut(index + 1)
            } else {
                base.as_ptr().sub(index * elem_layout.size())
            };

            Self {
                ptr: NonNull::new_unchecked(ptr as *mut ()),
            }
        }
    }

    unsafe fn next_n(&self, offset: usize, elem_layout: Layout) -> Self {
        let ptr = if elem_layout.size() == 0 {
            unsafe { NonNull::new_unchecked(invalid_mut(self.ptr.as_ptr() as usize + offset)) }
        } else {
            unsafe { self.ptr.byte_sub(offset * elem_layout.size()) }
        };
        Self { ptr }
    }

    unsafe fn drop(&mut self, props: Properties) {
        unsafe { (props.drop_fn)(self.ptr.as_ptr() as *mut u8) }
    }
}

pub(crate) fn invalid_mut<T>(addr: usize) -> *mut T {
    unsafe { core::mem::transmute(addr) }
}

#[allow(unused)]
#[derive(Clone, Copy)]
pub(super) enum Fallibilty {
    Fallible,
    Infallible,
}

#[allow(unused)]
#[derive(Clone, Copy)]
pub(super) struct Properties {
    pub is_zst: bool,
    pub needs_drop: bool,
    pub layout: Layout,
    pub table_layout: TableLayout,
    pub drop_fn: unsafe fn(*mut u8),
    pub array_layout: fn(usize) -> Result<Layout, LayoutError>,
}

pub(super) trait SizedTypeProps: Sized {
    const PROPERTIES: Properties = Properties {
        is_zst: mem::size_of::<Self>() == 0,
        needs_drop: mem::needs_drop::<Self>(),
        layout: Layout::new::<Self>(),
        table_layout: TableLayout::new::<Self>(),
        drop_fn: |ptr| unsafe {
            ptr::drop_in_place(ptr as *mut Self);
        },
        array_layout: Layout::array::<Self>,
    };
}

impl<T> SizedTypeProps for T {}

impl Fallibilty {
    fn capacity_overflow(self, layout: Layout) -> AllocError {
        match self {
            Fallibilty::Fallible => AllocError::SizeTooLarge { layout },
            Fallibilty::Infallible => panic!("Hash table capacity error"),
        }
    }

    fn alloc_error(self, layout: Layout) -> AllocError {
        match self {
            Self::Fallible => AllocError::OutOfMemory { layout },
            Self::Infallible => panic!(),
        }
    }
}

#[derive(Clone, Copy)]
struct Metadata {
    bucket_mask: usize,
    growth_left: usize,
    items: usize,
}

impl Metadata {
    const fn bucket_mask_to_capacity(self) -> usize {
        bucket_mask_to_capacity(self.bucket_mask)
    }

    const fn buckets(self) -> usize {
        self.bucket_mask + 1
    }

    const fn new(buckets: usize) -> Self {
        Self {
            bucket_mask: buckets - 1,
            items: 0,
            growth_left: bucket_mask_to_capacity(buckets - 1),
        }
    }

    const fn n_ctrl_bytes(self) -> usize {
        self.bucket_mask + 1 + Group::WIDTH
    }
}

#[derive(Clone)]
struct ProbeSeq {
    pos: usize,
    stride: usize,
}

impl ProbeSeq {
    const fn move_next(&mut self, bucket_mask: usize) {
        self.stride += Group::WIDTH;
        self.pos += self.stride;
        self.pos &= bucket_mask;
    }
}

#[derive(Clone, Copy)]
pub(super) struct TableLayout {
    size: usize,
    ctrl_align: usize,
}

impl TableLayout {
    const fn new<T>() -> Self {
        Self {
            size: T::PROPERTIES.layout.size(),
            ctrl_align: if T::PROPERTIES.layout.align() > Group::WIDTH {
                T::PROPERTIES.layout.align()
            } else {
                Group::WIDTH
            },
        }
    }

    const fn calculate_layout_for(self, buckets: usize) -> Option<(Layout, usize)> {
        debug_assert!(buckets.is_power_of_two());

        macro_rules! leap {
            ($x:expr) => {
                match $x {
                    Some(v) => v,
                    None => return None,
                }
            };
        }

        let TableLayout { size, ctrl_align } = self;

        let ctrl_offset =
            leap!(leap!(size.checked_mul(buckets)).checked_add(ctrl_align - 1)) & !(ctrl_align - 1);
        let len = leap!(ctrl_offset.checked_add(buckets + Group::WIDTH));

        if len > isize::MAX as usize - (ctrl_align - 1) {
            return None;
        }

        Some(unsafe {
            (
                Layout::from_size_align_unchecked(len, ctrl_align),
                ctrl_offset,
            )
        })
    }
}

const fn capacity_to_buckets(cap: usize, table_layout: TableLayout) -> Option<usize> {
    if cap < 15 {
        let min_cap = match (Group::WIDTH, table_layout.size) {
            (16, 0..=1) => 14,
            (16, 2..=3) => 7,
            (8, 0..=1) => 7,
            _ => 3,
        };
        let cap = if cap > min_cap { cap } else { min_cap };

        return Some(if cap < 4 {
            4
        } else if cap < 8 {
            8
        } else {
            16
        });
    }

    let adjusted_cap = match cap.checked_mul(8) {
        Some(v) => v,
        None => return None,
    } / 7;
    Some(adjusted_cap.next_power_of_two())
}

fn prev_pow2(z: usize) -> usize {
    let shift = mem::size_of::<usize>() * 8 - 1;
    1 << (shift - (z.leading_zeros() as usize))
}

fn max_buckets_in(alloc_size: usize, table_layout: TableLayout, group_width: usize) -> usize {
    let x = (alloc_size - group_width) / (table_layout.size + 1);
    prev_pow2(x)
}

const fn bucket_mask_to_capacity(bucket_mask: usize) -> usize {
    if bucket_mask < 8 {
        bucket_mask
    } else {
        ((bucket_mask + 1) / 8) * 7
    }
}

impl RawTableInner {
    pub(super) const NEW: Self = RawTableInner::empty();

    unsafe fn bucket(&self, index: usize, elem_layout: Layout) -> OpaqueBucket {
        unsafe { OpaqueBucket::from_base_index(self.data, index, elem_layout) }
    }

    unsafe fn find_inner(&self, hash: u64, eq: &mut dyn FnMut(usize) -> bool) -> Option<usize> {
        let tag_hash = Tag::full(hash);
        let mut probe_seq = self.probe_seq(hash);

        loop {
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };

            for bit in group.match_tag(tag_hash) {
                let index = (probe_seq.pos + bit) & self.metadata.bucket_mask;

                if likely(eq(index)) {
                    return Some(index);
                }
            }

            if likely(group.match_empty().any_bit_set()) {
                return None;
            }

            probe_seq.move_next(self.metadata.bucket_mask);
        }
    }

    unsafe fn find_or_find_insert_index_inner(
        &self,
        hash: u64,
        eq: &mut dyn FnMut(usize) -> bool,
    ) -> Result<usize, usize> {
        let mut insert_index = None;
        let tag_hash = Tag::full(hash);
        let mut probe_seq = self.probe_seq(hash);

        loop {
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };

            for bit in group.match_tag(tag_hash) {
                let index = (probe_seq.pos + bit) & self.metadata.bucket_mask;

                if likely(eq(index)) {
                    return Ok(index);
                }
            }

            if likely(insert_index.is_none()) {
                insert_index = self.find_insert_index_in_group(&group, &probe_seq);
            }

            if let Some(insert_index) = insert_index
                && likely(group.match_empty().any_bit_set())
            {
                unsafe {
                    return Err(self.fix_insert_index(insert_index));
                }
            }

            probe_seq.move_next(self.metadata.bucket_mask);
        }
    }

    unsafe fn record_item_insert_at(&mut self, index: usize, old_ctrl: Tag, new_ctrl: Tag) {
        unsafe {
            self.metadata.growth_left -= usize::from(old_ctrl.special_is_empty());
            self.set_ctrl(index, new_ctrl);
            self.metadata.items += 1;
        }
    }

    const fn empty() -> RawTableInner {
        const {
            Self {
                data: unsafe {
                    NonNull::new_unchecked(Group::static_empty().as_ptr().cast_mut().cast())
                },
                metadata: Metadata {
                    bucket_mask: 0,
                    growth_left: 0,
                    items: 0,
                },
            }
        }
    }

    const fn n_ctrl_bytes(&self) -> usize {
        self.metadata.n_ctrl_bytes()
    }

    const fn ctrl_slice(&mut self) -> &mut [Tag] {
        unsafe { slice::from_raw_parts_mut(self.data.as_ptr().cast(), self.n_ctrl_bytes()) }
    }

    fn fallible_with_capacity(
        alloc: &(dyn Allocator + '_),
        table_layout: TableLayout,
        cap: usize,
        falliblilty: Fallibilty,
    ) -> Result<Self, AllocError> {
        if cap == 0 {
            Ok(Self::NEW)
        } else {
            unsafe {
                let buckets = capacity_to_buckets(cap, table_layout)
                    .ok_or_else(|| falliblilty.capacity_overflow(Layout::new::<u8>()))?;
                let mut result =
                    Self::new_uninitialized(alloc, table_layout, buckets, falliblilty)?;
                result.ctrl_slice().fill_empty();
                Ok(result)
            }
        }
    }

    unsafe fn new_uninitialized(
        alloc: &(dyn Allocator + '_),
        table_layout: TableLayout,
        mut buckets: usize,
        falliblilty: Fallibilty,
    ) -> Result<Self, AllocError> {
        debug_assert!(buckets.is_power_of_two());

        let (layout, mut ctrl_offset) = match table_layout.calculate_layout_for(buckets) {
            Some(lco) => lco,
            None => return Err(falliblilty.capacity_overflow(Layout::PROPERTIES.layout)),
        };

        let ptr: NonNull<u8> = match unsafe { alloc.try_allocate(layout) } {
            Ok(block) => {
                if block.len() != layout.size() {
                    let x = max_buckets_in(block.len(), table_layout, Group::WIDTH);
                    debug_assert!(x >= buckets);

                    let (_, oversized_ctrl_offset) = match table_layout.calculate_layout_for(x) {
                        Some(lco) => lco,
                        None => unsafe { hint::unreachable_unchecked() },
                    };

                    ctrl_offset = oversized_ctrl_offset;
                    buckets = x;
                }

                block.cast()
            }
            Err(_) => return Err(falliblilty.alloc_error(layout)),
        };

        let ctrl = unsafe { ptr.add(ctrl_offset) };

        Ok(Self {
            data: ctrl,
            metadata: Metadata::new(buckets),
        })
    }

    const fn buckets(&self) -> usize {
        self.metadata.buckets()
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn reserve_rehash_inner(
        &mut self,
        // dyn is used here for optimizations
        alloc: &(dyn Allocator + '_),
        additional: usize,
        hasher: &(dyn Fn(&mut Self, usize) -> u64 + '_),
        falliblilty: Fallibilty,
        layout: TableLayout,
        elem_layout: Layout,
        size_of_elem: usize,
        drop: Option<unsafe fn(*mut u8)>,
    ) -> Result<(), AllocError> {
        let new_items = match self.metadata.items.checked_add(additional) {
            Some(new_items) => new_items,
            None => {
                return Err(falliblilty.capacity_overflow(unsafe {
                    Layout::from_size_align_unchecked(
                        additional * elem_layout.size(),
                        layout.ctrl_align,
                    )
                }));
            }
        };

        let full_capacity = self.metadata.bucket_mask_to_capacity();

        unsafe {
            if new_items <= full_capacity / 2 {
                self.rehash_in_place(hasher, size_of_elem, drop);
                Ok(())
            } else {
                self.resize_inner(
                    alloc,
                    usize::max(new_items, full_capacity + 1),
                    hasher,
                    falliblilty,
                    layout,
                )
            }
        }
    }

    pub(super) unsafe fn drop_inner(
        &mut self,
        alloc: &(dyn Allocator + '_),
        type_properties: Properties,
    ) {
        if !self.is_empty_singleton() {
            unsafe {
                self.drop_elements(type_properties);

                self.free_buckets(alloc, type_properties.table_layout);
            }
        }
    }

    fn is_empty_singleton(&self) -> bool {
        self.metadata.bucket_mask == 0
    }

    #[allow(clippy::type_complexity)]
    fn prep_resize<'a>(
        &self,
        alloc: &'a (dyn Allocator + 'a),
        table_layout: TableLayout,
        cap: usize,
        falliblilty: Fallibilty,
    ) -> Result<
        guard::ScopeGuard<impl FnOnce(ManuallyDrop<Self>) + 'a, ManuallyDrop<Self>>,
        AllocError,
    > {
        debug_assert!(self.metadata.items <= cap);

        let new_table =
            RawTableInner::fallible_with_capacity(alloc, table_layout, cap, falliblilty)?;
        Ok(guard::guard(
            ManuallyDrop::new(new_table),
            move |mut self_| {
                if !self_.is_empty_singleton() {
                    unsafe {
                        self_.free_buckets(alloc, table_layout);
                    }
                }
            },
        ))
    }

    unsafe fn free_buckets(&mut self, alloc: &(dyn Allocator + '_), table_layout: TableLayout) {
        unsafe {
            let (ptr, layout) = self.alloc_info(table_layout);
            alloc.deallocate(layout, ptr);
        }
    }

    unsafe fn alloc_info(&self, table_layout: TableLayout) -> (NonNull<u8>, Layout) {
        let (layout, ctrl_offset) = match table_layout.calculate_layout_for(self.buckets()) {
            Some(lco) => lco,
            None => unsafe { hint::unreachable_unchecked() },
        };
        (unsafe { self.data.sub(ctrl_offset) }, layout)
    }

    unsafe fn resize_inner(
        &mut self,
        alloc: &(dyn Allocator + '_),
        cap: usize,
        hasher: &(dyn Fn(&mut Self, usize) -> u64 + '_),
        falliblilty: Fallibilty,
        layout: TableLayout,
    ) -> Result<(), AllocError> {
        let mut new_table = self.prep_resize(alloc, layout, cap, falliblilty)?;

        unsafe {
            for full_byte_index in self.full_buckets_indices() {
                let hash = hasher(self, full_byte_index);

                let (new_index, _) = new_table.prep_insert_index(hash);

                ptr::copy_nonoverlapping(
                    self.bucket_ptr(full_byte_index, layout.size),
                    new_table.bucket_ptr(new_index, layout.size),
                    layout.size,
                );
            }

            new_table.metadata.growth_left -= self.metadata.items;
            new_table.metadata.items = self.metadata.items;
            mem::swap(self, &mut **new_table);

            Ok(())
        }
    }

    unsafe fn prep_insert_index(&mut self, hash: u64) -> (usize, Tag) {
        unsafe {
            let index = self.find_insert_index(hash);

            let old = *self.ctrl(index);
            self.set_ctrl_hash(index, hash);
            (index, old)
        }
    }

    const unsafe fn ctrl(&self, index: usize) -> *mut Tag {
        unsafe { self.data.as_ptr().add(index).cast() }
    }

    const unsafe fn prep_rehash_in_place(&mut self) {
        let mut group_index = 0;

        while group_index < self.buckets() {
            let group = unsafe { Group::load_aligned(self.ctrl(group_index)) };
            let group = group.convert_special_to_empty_and_full_to_deleted();
            unsafe { group.store_aligned(self.ctrl(group_index)) }
            group_index += Group::WIDTH - 1;
        }

        if unlikely(self.buckets() < Group::WIDTH) {
            unsafe {
                self.ctrl(0)
                    .copy_to(self.ctrl(Group::WIDTH), self.buckets());
            }
        } else {
            unsafe {
                self.ctrl(0)
                    .copy_to(self.ctrl(self.buckets()), Group::WIDTH);
            }
        }
    }

    unsafe fn rehash_in_place(
        &mut self,
        hasher: &(dyn Fn(&mut Self, usize) -> u64 + '_),
        size_of_elem: usize,
        drop: Option<unsafe fn(*mut u8)>,
    ) {
        unsafe {
            self.prep_rehash_in_place();

            let mut guard = guard::guard(self, move |self_| {
                if let Some(drop) = drop {
                    for i in 0..self_.buckets() {
                        if *self_.ctrl(i) == Tag::DELETED {
                            self_.set_ctrl(i, Tag::EMPTY);
                            drop(self_.bucket_ptr(i, size_of_elem));
                            self_.metadata.items -= 1;
                        }
                    }
                }
                self_.metadata.growth_left =
                    self_.metadata.bucket_mask_to_capacity() - self_.metadata.items;
            });

            'outer: for i in 0..guard.buckets() {
                if *guard.ctrl(i) != Tag::DELETED {
                    continue;
                }

                let i_p = guard.bucket_ptr(i, size_of_elem);

                'inner: loop {
                    let hash = hasher(*guard, i);

                    let new_i = guard.find_insert_index(hash);

                    if likely(guard.is_in_same_group(i, new_i, hash)) {
                        guard.set_ctrl_hash(i, hash);
                        continue 'outer;
                    }

                    let new_i_p = guard.bucket_ptr(new_i, size_of_elem);

                    let prev_ctrl = guard.replace_ctrl_hash(new_i, hash);

                    if prev_ctrl == Tag::EMPTY {
                        guard.set_ctrl(i, Tag::EMPTY);

                        ptr::copy_nonoverlapping(i_p, new_i_p, size_of_elem);
                        continue 'outer;
                    } else {
                        debug_assert_eq!(prev_ctrl, Tag::DELETED);
                        ptr::swap_nonoverlapping(i_p, new_i_p, size_of_elem);
                        continue 'inner;
                    }
                }
            }

            guard.metadata.growth_left = guard.metadata.bucket_mask_to_capacity();

            mem::forget(guard);
        }
    }

    unsafe fn set_ctrl_hash(&mut self, index: usize, hash: u64) {
        unsafe {
            self.set_ctrl(index, Tag::full(hash));
        }
    }

    unsafe fn replace_ctrl_hash(&mut self, index: usize, hash: u64) -> Tag {
        unsafe { ptr::replace(self.ctrl(index), Tag::full(hash)) }
    }

    fn is_in_same_group(&self, i: usize, new_i: usize, hash: u64) -> bool {
        let probe_seq_pos = self.probe_seq(hash).pos;
        let probe_index = |pos: usize| {
            (pos.wrapping_sub(probe_seq_pos) & self.metadata.bucket_mask) / Group::WIDTH
        };

        probe_index(i) == probe_index(new_i)
    }

    const unsafe fn fix_insert_index(&self, mut index: usize) -> usize {
        unsafe {
            if unlikely(self.is_bucket_full(index)) {
                debug_assert!(self.metadata.bucket_mask < Group::WIDTH);

                index = Group::load_aligned(self.ctrl(0))
                    .match_empty_or_deleted()
                    .lowest_bit_set()
                    .unwrap_unchecked()
            }

            index
        }
    }

    const unsafe fn is_bucket_full(&self, index: usize) -> bool {
        unsafe { (*self.ctrl(index)).is_full() }
    }

    unsafe fn find_insert_index(&self, hash: u64) -> usize {
        let mut probe_seq = self.probe_seq(hash);
        loop {
            let group = unsafe { Group::load(self.ctrl(probe_seq.pos)) };
            let index = self.find_insert_index_in_group(&group, &probe_seq);

            if likely(index.is_some()) {
                unsafe {
                    return self.fix_insert_index(index.unwrap_unchecked());
                }
            }
            probe_seq.move_next(self.metadata.bucket_mask);
        }
    }

    const fn find_insert_index_in_group(
        &self,
        group: &Group,
        probe_seq: &ProbeSeq,
    ) -> Option<usize> {
        let bit = group.match_empty_or_deleted().lowest_bit_set();
        if likely(bit.is_some()) {
            Some((probe_seq.pos + bit.unwrap()) & self.metadata.bucket_mask)
        } else {
            None
        }
    }

    fn probe_seq(&self, hash: u64) -> ProbeSeq {
        ProbeSeq {
            pos: hash as usize & self.metadata.bucket_mask,
            stride: 0,
        }
    }

    unsafe fn set_ctrl(&mut self, index: usize, ctrl: Tag) {
        let index2 =
            ((index.wrapping_sub(Group::WIDTH)) & self.metadata.bucket_mask) + Group::WIDTH;
        unsafe {
            *self.ctrl(index) = ctrl;
            *self.ctrl(index2) = ctrl;
        }
    }

    fn data_end<T>(&self) -> NonNull<T> {
        self.data.cast()
    }

    unsafe fn bucket_ptr(&self, index: usize, size_of: usize) -> *mut u8 {
        debug_assert_ne!(self.metadata.bucket_mask, 0);
        debug_assert!(index < self.buckets());
        let base: *mut u8 = self.data_end().as_ptr();
        unsafe { base.sub((index + 1) * size_of) }
    }

    unsafe fn full_buckets_indices(&self) -> FullBucketsIndices {
        let ctrl = unsafe { NonNull::new_unchecked(self.ctrl(0).cast::<u8>()) };

        unsafe {
            FullBucketsIndices {
                current_group: Group::load_aligned(ctrl.as_ptr().cast())
                    .match_full()
                    .into_iter(),
                group_first_index: 0,
                ctrl,
                items: self.metadata.items,
            }
        }
    }

    pub(super) unsafe fn iter(&self, type_properties: Properties) -> OpaqueRawIter {
        unsafe {
            let data = OpaqueBucket::from_base_index(self.data, 0, type_properties.layout);
            OpaqueRawIter {
                iter: OpaqueRawIterRange::new(self.data.as_ptr(), data, self.buckets()),
                items: self.metadata.items,
                elem_layout: type_properties.layout,
            }
        }
    }

    pub(super) unsafe fn drop_elements(&mut self, type_properties: Properties) {
        if type_properties.needs_drop && self.metadata.items != 0 {
            unsafe {
                for mut item in self.iter(type_properties) {
                    item.drop(type_properties);
                }
            }
        }
    }
}

struct OpaqueRawIter {
    iter: OpaqueRawIterRange,
    items: usize,
    elem_layout: Layout,
}

impl Iterator for OpaqueRawIter {
    type Item = OpaqueBucket;

    fn next(&mut self) -> Option<Self::Item> {
        if self.items == 0 {
            return None;
        }

        let nxt = unsafe { self.iter.next_impl::<false>(self.elem_layout) };

        debug_assert!(nxt.is_some());
        self.items -= 1;

        nxt
    }
}

struct OpaqueRawIterRange {
    current_group: BitMaskIter,
    data: OpaqueBucket,
    next_ctrl: *const u8,
    end: *const u8,
}

impl OpaqueRawIterRange {
    unsafe fn new(ctrl: *const u8, data: OpaqueBucket, len: usize) -> Self {
        unsafe {
            let end = ctrl.add(len);

            let current = Group::load_aligned(ctrl.cast()).match_full();
            let next_ctrl = ctrl.add(Group::WIDTH);

            Self {
                current_group: current.into_iter(),
                data,
                next_ctrl,
                end,
            }
        }
    }

    unsafe fn next_impl<const DO_CHECK_PTR_RANGE: bool>(
        &mut self,
        elem_layout: Layout,
    ) -> Option<OpaqueBucket> {
        loop {
            unsafe {
                if let Some(index) = self.current_group.next() {
                    return Some(self.data.next_n(index, elem_layout));
                }

                if DO_CHECK_PTR_RANGE && self.next_ctrl >= self.end {
                    return None;
                }

                self.current_group = Group::load_aligned(self.next_ctrl.cast())
                    .match_full()
                    .into_iter();
                self.data = self.data.next_n(Group::WIDTH, elem_layout);
                self.next_ctrl = self.next_ctrl.add(Group::WIDTH)
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct FullBucketsIndices {
    current_group: BitMaskIter,
    group_first_index: usize,
    ctrl: NonNull<u8>,
    items: usize,
}

impl FullBucketsIndices {
    unsafe fn next_impl(&mut self) -> Option<usize> {
        loop {
            if let Some(index) = self.current_group.next() {
                return Some(self.group_first_index + index);
            }
            unsafe {
                self.ctrl = self.ctrl.add(Group::WIDTH);
                self.current_group = Group::load_aligned(self.ctrl.as_ptr().cast())
                    .match_full()
                    .into_iter();
                self.group_first_index += Group::WIDTH;
            }
        }
    }
}

impl Iterator for FullBucketsIndices {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.items == 0 {
            return None;
        }

        let nxt = unsafe { self.next_impl() };

        debug_assert!(nxt.is_some());
        self.items -= 1;
        nxt
    }
}

#[inline]
const fn likely(cond: bool) -> bool {
    if cond {
        true
    } else {
        core::hint::cold_path();
        false
    }
}

#[inline]
const fn unlikely(cond: bool) -> bool {
    if !cond {
        false
    } else {
        core::hint::cold_path();
        true
    }
}

pub(crate) mod guard {
    use core::{
        mem::ManuallyDrop,
        ops::{Deref, DerefMut},
    };

    pub struct ScopeGuard<F, T>
    where
        F: FnOnce(T),
    {
        val: ManuallyDrop<T>,
        fun: ManuallyDrop<F>,
    }

    pub const fn guard<F, T>(value: T, f: F) -> ScopeGuard<F, T>
    where
        F: FnOnce(T),
    {
        ScopeGuard {
            val: ManuallyDrop::new(value),
            fun: ManuallyDrop::new(f),
        }
    }

    impl<T, F> ScopeGuard<F, T>
    where
        F: FnOnce(T),
    {
        #[allow(unused)]
        pub fn into_inner(this: Self) -> T {
            let mut this = ManuallyDrop::new(this);

            unsafe {
                let value = ManuallyDrop::take(&mut this.val);
                ManuallyDrop::drop(&mut this.fun);
                value
            }
        }
    }

    impl<T, F> Deref for ScopeGuard<F, T>
    where
        F: FnOnce(T),
    {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.val
        }
    }

    impl<T, F> DerefMut for ScopeGuard<F, T>
    where
        F: FnOnce(T),
    {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.val
        }
    }

    impl<T, F> Drop for ScopeGuard<F, T>
    where
        F: FnOnce(T),
    {
        fn drop(&mut self) {
            let ScopeGuard { val, fun } = self;

            let dropfn = unsafe { ManuallyDrop::take(fun) };
            let value = unsafe { ManuallyDrop::take(val) };

            (dropfn)(value)
        }
    }
}

pub(crate) mod tag {
    use core::{fmt, mem};

    use crate::collections::map::word::Word;

    #[derive(Clone, Copy, PartialEq, Eq)]
    #[repr(transparent)]
    pub(crate) struct Tag(pub(crate) u8);

    impl Tag {
        pub(crate) const EMPTY: Tag = Tag(0x7f);
        pub(crate) const DELETED: Tag = Tag(0);

        pub(crate) const fn is_full(self) -> bool {
            self.0 & 0x80 == 0
        }

        pub(crate) const fn is_special(self) -> bool {
            self.0 & 0x80 != 0
        }

        pub(crate) const fn special_is_empty(self) -> bool {
            debug_assert!(self.is_special());
            self.0 & 0x01 != 0
        }

        pub(crate) const fn full(hash: u64) -> Tag {
            const MIN_HASH_LEN: usize = if mem::size_of::<usize>() < mem::size_of::<u64>() {
                mem::size_of::<usize>()
            } else {
                mem::size_of::<u64>()
            };

            let top7 = hash >> (MIN_HASH_LEN * 8 - 7);
            Tag((top7 & 0x7f) as u8)
        }
    }

    impl fmt::Debug for Tag {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if self.is_special() {
                if self.special_is_empty() {
                    f.pad("EMPTY")
                } else {
                    f.pad("DELETED")
                }
            } else {
                f.debug_tuple("full").field(&(self.0 & 0x7F)).finish()
            }
        }
    }

    pub(crate) trait TagSliceExt {
        fn fill(&mut self, tag: Tag);

        #[inline]
        fn fill_empty(&mut self) {
            self.fill(Tag::EMPTY);
        }
    }

    pub(crate) const fn repeat(Tag(inner): Tag) -> Word {
        Word::from_ne_bytes([inner; _])
    }

    impl TagSliceExt for [Tag] {
        fn fill(&mut self, tag: Tag) {
            unsafe {
                self.as_mut_ptr().write_bytes(tag.0, self.len());
            }
        }
    }
}

pub(crate) mod group {
    use core::{mem, ptr};

    use crate::collections::map::bitmask::BitMask;
    use crate::collections::map::tag::{self, Tag, repeat};
    use crate::collections::map::word::Word as GroupWord;

    #[derive(Clone, Copy)]
    pub(crate) struct Group(GroupWord);

    impl Group {
        pub(crate) const WIDTH: usize = { mem::size_of::<GroupWord>() };

        pub(crate) const fn static_empty() -> &'static [tag::Tag; Group::WIDTH] {
            #[repr(C)]
            struct AlignedTags {
                _align: [Group; 0],
                tags: [Tag; Group::WIDTH],
            }

            const ALIGNED: AlignedTags = AlignedTags {
                _align: [],
                tags: [Tag::EMPTY; Group::WIDTH],
            };
            &ALIGNED.tags
        }

        pub(crate) const unsafe fn load(ptr: *const Tag) -> Self {
            Self(unsafe { ptr::read_unaligned(ptr as *const GroupWord) })
        }

        pub(crate) const unsafe fn load_aligned(ptr: *const Tag) -> Self {
            Self(unsafe { ptr::read(ptr as *const GroupWord) })
        }

        pub(crate) const unsafe fn store_aligned(self, ptr: *mut Tag) {
            unsafe { ptr::write(ptr as *mut GroupWord, self.0) }
        }

        #[allow(unused)]
        pub(crate) const fn match_tag(self, tag: Tag) -> BitMask {
            let cmp = self.0 ^ repeat(tag);
            BitMask((cmp.wrapping_sub(repeat(Tag(0x01))) & !cmp & repeat(Tag::DELETED)).to_le())
        }

        #[allow(unused)]
        pub(crate) const fn match_empty(self) -> BitMask {
            BitMask((self.0 & (self.0 << 1) & repeat(Tag::DELETED)).to_le())
        }

        pub(crate) const fn match_empty_or_deleted(self) -> BitMask {
            BitMask((self.0 & repeat(Tag::DELETED)).to_le())
        }

        pub(crate) const fn match_full(self) -> BitMask {
            self.match_empty_or_deleted().invert()
        }

        pub(crate) const fn convert_special_to_empty_and_full_to_deleted(self) -> Self {
            let full = !self.0 & repeat(Tag::DELETED);
            Group(!full + (full >> 7))
        }
    }
}

pub(crate) mod word {
    use core::num::NonZero;

    #[cfg(any(target_pointer_width = "64", target_arch = "wasm32"))]
    pub(crate) type Word = u64;

    #[cfg(not(any(target_pointer_width = "64", target_arch = "wasm32")))]
    pub(crate) type Word = u32;

    pub(crate) type NonZeroWord = NonZero<Word>;

    pub(crate) const BITMASK_MASK: Word = u64::from_ne_bytes([128; 8]) as Word;
    pub(crate) const BITMASK_STRIDE: usize = 8;
    pub(crate) const BITMASK_ITER_MASK: Word = !0;
}

pub(crate) mod bitmask {
    use crate::collections::map::word::{
        self, BITMASK_ITER_MASK, BITMASK_MASK, BITMASK_STRIDE, NonZeroWord,
    };

    #[derive(Clone, Copy)]
    pub(crate) struct BitMask(pub(crate) word::Word);

    impl BitMask {
        pub(crate) const fn invert(self) -> Self {
            BitMask(self.0 ^ BITMASK_MASK)
        }

        const fn remove_lowest_bit(self) -> Self {
            BitMask(self.0 & (self.0 - 1))
        }

        #[allow(unused)]
        pub(crate) const fn any_bit_set(self) -> bool {
            self.0 != 0
        }

        pub(crate) const fn lowest_bit_set(self) -> Option<usize> {
            if let Some(nonzero) = NonZeroWord::new(self.0) {
                Some(Self::nonzero_trailing_zeros(nonzero))
            } else {
                None
            }
        }

        #[allow(unused)]
        #[allow(clippy::manual_is_multiple_of)]
        pub(crate) const fn trailing_zeros(self) -> usize {
            if cfg!(target_arch = "arm") && BITMASK_STRIDE % 8 == 0 {
                self.0.swap_bytes().leading_zeros() as usize / BITMASK_STRIDE
            } else {
                self.0.trailing_zeros() as usize / BITMASK_STRIDE
            }
        }

        #[allow(clippy::manual_is_multiple_of)]
        const fn nonzero_trailing_zeros(nonzero: NonZeroWord) -> usize {
            if cfg!(target_arch = "arm") && BITMASK_STRIDE % 8 == 0 {
                let swapped = unsafe { NonZeroWord::new_unchecked(nonzero.get().swap_bytes()) };
                swapped.leading_zeros() as usize / BITMASK_STRIDE
            } else {
                nonzero.trailing_zeros() as usize / BITMASK_STRIDE
            }
        }

        #[allow(unused)]
        pub(crate) const fn leading_zeros(self) -> usize {
            self.0.leading_zeros() as usize / BITMASK_STRIDE
        }
    }

    impl IntoIterator for BitMask {
        type Item = usize;
        type IntoIter = BitMaskIter;

        fn into_iter(self) -> Self::IntoIter {
            BitMaskIter(BitMask(self.0 & BITMASK_ITER_MASK))
        }
    }

    #[derive(Clone)]
    pub(crate) struct BitMaskIter(pub(crate) BitMask);

    impl Iterator for BitMaskIter {
        type Item = usize;

        fn next(&mut self) -> Option<Self::Item> {
            let bit = self.0.lowest_bit_set()?;
            self.0 = self.0.remove_lowest_bit();
            Some(bit)
        }
    }
}
