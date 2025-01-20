#[allow(unused)]
pub trait Merge<O> {
    fn merge(self) -> O;
}

impl<O> Merge<O> for Result<O, O> {
    fn merge(self) -> O {
        match self {
            Ok(o) => o,
            Err(o) => o,
        }
    }
}

#[macro_export]
macro_rules! user_err {
    ($w:expr, $e:expr) => {
        warn!("{}:{}:{} => {}", file!(), line!(), $w, $e)
    };
}
