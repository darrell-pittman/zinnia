pub trait LossyFrom<T: Sized>: Sized {
    fn lossy_from(_: T) -> Self;
}

#[macro_export]
macro_rules! impl_lossy_from {
    ($from:ty;$($ty:ty)*) => {
        $(
            impl LossyFrom<$from> for $ty {
                #[inline]
                fn lossy_from(v: $from) -> $ty {
                    v as $ty
                }
            }
        )*
    }
}
