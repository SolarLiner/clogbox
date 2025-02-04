use crate::storage::SRef;
use crate::ParamBuffer;

pub trait ParamStorage {
    fn get_param_buffer_in(&self, index: usize) -> SRef<&ParamBuffer>;
    fn get_param_buffer_out(&self, index: usize) -> SRef<&mut ParamBuffer>;
}
