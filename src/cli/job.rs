#[derive(Debug, Eq, PartialEq)]
pub enum Job {
    Uid(usize),
    Fingerprint(u64),
    Tag(String),
}
