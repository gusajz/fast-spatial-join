use std::collections::HashMap;

pub type PropertyMap = HashMap<String, String>;

#[derive(Debug)]
pub struct FindResult<'a> {
    pub props: &'a PropertyMap,
    pub distance: f64,
}
