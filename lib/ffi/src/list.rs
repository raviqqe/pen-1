use crate::{Any, Arc, BoxAny, Closure};

extern "C" {
    fn pen_ffi_list_create() -> Arc<List>;
    fn pen_ffi_list_prepend(x: BoxAny, xs: Arc<List>) -> Arc<List>;
}

#[pen_ffi_macro::any(crate = "crate")]
#[repr(C)]
#[derive(Clone)]
pub struct List {
    node: Arc<Closure>,
}

impl List {
    pub fn new() -> Arc<Self> {
        unsafe { pen_ffi_list_create() }
    }

    pub fn prepend(this: Arc<Self>, x: impl Into<Any>) -> Arc<Self> {
        unsafe { pen_ffi_list_prepend(x.into().into(), this) }
    }
}

impl Default for Arc<List> {
    fn default() -> Self {
        List::new()
    }
}
