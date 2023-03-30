use std::fmt::Display;

pub trait HasId {
    const TYPE: &'static str;
    type Id<'a>: Display + Clone + Copy
    where
        Self: 'a;
    fn id(&self) -> Self::Id<'_>;
}
