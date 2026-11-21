/// Where fetched rows come from: a Folder of this `uid_validity`, of the
/// Account whose address is `email`.
#[derive(Clone, Copy)]
pub struct Origin<'a> {
    pub uid_validity: u32,
    pub email: &'a str,
}
