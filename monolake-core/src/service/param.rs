pub trait Param<T> {
    fn param(&self) -> T;
}

impl<T: Clone> Param<T> for T {
    fn param(&self) -> T {
        self.clone()
    }
}
