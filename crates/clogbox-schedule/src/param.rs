use crate::ParamBuffer;
use crate::storage::StorageBorrow;

pub trait ParamStorage {
    fn get_param_buffer_in(&self, index: usize) -> StorageBorrow<&ParamBuffer>;
    fn get_param_buffer_out(&self, index: usize) -> StorageBorrow<&mut ParamBuffer>;
}
