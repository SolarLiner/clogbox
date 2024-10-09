mod value;

use crate::param::value::Value;
use crate::r#enum::Enum;

pub trait GetParameter {
    type Param: Enum;
    
    fn get_param_raw(&self, param: Self::Param) -> Value;
    
    fn get_param_as<'a, V: 'a + TryFrom<Value<'a>>>(&'a self, param: Self::Param) -> Result<V, V::Error> {
        self.get_param_raw(param).try_into()
    }
}

pub trait SetParameter {
    type Param: Enum;
    
    fn set_param_raw(&mut self, param: Self::Param, value: Value);
    
    fn set_param<'a>(&mut self, param: Self::Param, value: impl Into<Value<'a>>) {
        self.set_param_raw(param, value.into())
    }
    fn set_param_fallible<'a, V: TryInto<Value<'a>>>(&mut self, param: Self::Param, value: V) -> Result<(), V::Error> {
        value.try_into().map(|value| self.set_param_raw(param, value))
    }
}

pub trait NormalizeParameter {
    type Param: Enum;
    
    fn normalize_param<'a>(&self, param: Self::Param, value: impl Into<Value<'a>>) -> Option<f32>;
    fn unnormalize_param<'a>(&self, param: Self::Param, value: f32) -> Option<Value<'a>>;
}