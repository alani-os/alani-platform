//! `paging` module boundary.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PagingDescriptor<'a> {
    pub name: &'a str,
    pub version: u32,
}

impl<'a> PagingDescriptor<'a> {
    pub const fn new(name: &'a str, version: u32) -> Self {
        Self { name, version }
    }
}
