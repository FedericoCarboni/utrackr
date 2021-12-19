use std::fmt;

pub struct AnnouncePkt<'a> {
    inner: &'a [u8],
}

impl<'a> fmt::Debug for AnnouncePkt<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnnouncePkt")
            .field("", &"".to_string())
            .finish()
    }
}
