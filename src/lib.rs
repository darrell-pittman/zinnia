pub mod convert;
pub mod error;
pub mod sound;

pub type Result<T> = std::result::Result<T, error::Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
