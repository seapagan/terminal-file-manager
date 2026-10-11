use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Close<'a> {
	pub(crate) id: u32,
	handle:        Cow<'a, [u8]>,
}

impl<'a> Close<'a> {
	pub(crate) fn new(handle: impl Into<Cow<'a, [u8]>>) -> Self {
		Self { id: 0, handle: handle.into() }
	}

	pub(crate) fn len(&self) -> usize { size_of_val(&self.id) + 4 + self.handle.len() }
}
