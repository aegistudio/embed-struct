use std::usize;

pub trait NullablePtr: Copy + Eq {
    fn nullptr() -> Self;

    fn null(&self) -> bool {
        *self == Self::nullptr()
    }

    fn to_option(&self) -> Option<Self> {
        if !self.null() { Some(*self) } else { None }
    }
}

impl NullablePtr for usize {
    fn nullptr() -> Self {
        usize::MAX
    }
}
