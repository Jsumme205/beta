#![no_std]

pub use update_macro::Update;

#[cfg(feature = "alloc")]
extern crate alloc;

pub trait UpdateField {
    type Update;

    fn update(&mut self, val: Self::Update);

    #[cfg(feature = "alloc")]
    fn batch_update(&mut self, updates: alloc::boxed::Box<[Self::Update]>) {
        for update in updates {
            self.update(update)
        }
    }
}

impl<U> UpdateField for &mut U
where
    U: UpdateField + ?Sized,
{
    type Update = U::Update;

    fn update(&mut self, val: Self::Update) {
        (*self).update(val);
    }
}

pub trait TryUpdateField {
    type Update;
    type Error;

    fn try_update(&mut self, val: Self::Update) -> Result<(), Self::Error>;
}

impl<U> TryUpdateField for U
where
    U: UpdateField + ?Sized,
{
    type Error = core::convert::Infallible;
    type Update = U::Update;

    fn try_update(&mut self, val: Self::Update) -> Result<(), Self::Error> {
        self.update(val);
        Ok(())
    }
}
