pub mod convert;
pub mod error;
pub mod hwp;
pub mod sound;

use crate::convert::LossyFrom;

pub type Result<T> = std::result::Result<T, error::Error>;
impl_lossy_from!(f32; i16 u16 i32 u32 i64 u64 f32 f64);

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
