//! A forgiving deserializer over [`ron::Value`] for registry component data.
//!
//! Compared to `Value::into_rust`, this adds the ergonomics data files rely on:
//! - **implicit `Some`**: `region: "hi"` works for an `Option<String>` field;
//! - **unit = default**: `"Transform2D": ()` deserializes a `#[serde(default)]` struct;
//! - **newtype from 1-element seq**: `"Name": ("player")` works for `Name(String)`.

use serde::de::{DeserializeSeed, Deserializer, IntoDeserializer, Visitor};
use serde::forward_to_deserialize_any;

type DeError = serde::de::value::Error;

pub(crate) struct FlexValue<'a>(pub &'a ron::Value);

struct FlexSeq<'a> {
    items: std::slice::Iter<'a, ron::Value>,
}

impl<'de> serde::de::SeqAccess<'de> for FlexSeq<'_> {
    type Error = DeError;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, DeError> {
        match self.items.next() {
            Some(value) => seed.deserialize(FlexValue(value)).map(Some),
            None => Ok(None),
        }
    }
}

struct FlexMap<'a> {
    entries: std::vec::IntoIter<(&'a ron::Value, &'a ron::Value)>,
    pending: Option<&'a ron::Value>,
}

impl<'de> serde::de::MapAccess<'de> for FlexMap<'_> {
    type Error = DeError;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, DeError> {
        match self.entries.next() {
            Some((key, value)) => {
                self.pending = Some(value);
                seed.deserialize(FlexValue(key)).map(Some)
            }
            None => Ok(None),
        }
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, DeError> {
        let value = self.pending.take().expect("value after key");
        seed.deserialize(FlexValue(value))
    }
}

impl<'de> Deserializer<'de> for FlexValue<'_> {
    type Error = DeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        use ron::value::Number;
        match self.0 {
            ron::Value::Bool(b) => visitor.visit_bool(*b),
            ron::Value::Char(c) => visitor.visit_char(*c),
            ron::Value::String(s) => visitor.visit_str(s),
            ron::Value::Bytes(b) => visitor.visit_bytes(b),
            ron::Value::Unit => visitor.visit_unit(),
            ron::Value::Option(Some(inner)) => visitor.visit_some(FlexValue(inner)),
            ron::Value::Option(None) => visitor.visit_none(),
            ron::Value::Number(number) => match number {
                Number::I8(v) => visitor.visit_i64(*v as i64),
                Number::I16(v) => visitor.visit_i64(*v as i64),
                Number::I32(v) => visitor.visit_i64(*v as i64),
                Number::I64(v) => visitor.visit_i64(*v),
                Number::U8(v) => visitor.visit_u64(*v as u64),
                Number::U16(v) => visitor.visit_u64(*v as u64),
                Number::U32(v) => visitor.visit_u64(*v as u64),
                Number::U64(v) => visitor.visit_u64(*v),
                other => visitor.visit_f64(other.into_f64()),
            },
            ron::Value::Seq(items) => visitor.visit_seq(FlexSeq {
                items: items.iter(),
            }),
            ron::Value::Map(map) => visitor.visit_map(FlexMap {
                entries: map.iter().collect::<Vec<_>>().into_iter(),
                pending: None,
            }),
        }
    }

    /// Implicit `Some`: any non-Option value inside an `Option` field is `Some(value)`.
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, DeError> {
        match self.0 {
            ron::Value::Option(Some(inner)) => visitor.visit_some(FlexValue(inner)),
            ron::Value::Option(None) | ron::Value::Unit => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    /// Unit = "all defaults" for `#[serde(default)]` structs.
    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        match self.0 {
            ron::Value::Unit => visitor.visit_map(FlexMap {
                entries: Vec::new().into_iter(),
                pending: None,
            }),
            _ => self.deserialize_any(visitor),
        }
    }

    /// Newtype structs accept `("inner")` (a 1-element seq) or the bare inner value.
    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        match self.0 {
            ron::Value::Seq(items) if items.len() == 1 => {
                visitor.visit_newtype_struct(FlexValue(&items[0]))
            }
            _ => visitor.visit_newtype_struct(self),
        }
    }

    /// Unit enum variants from strings; externally-tagged variants from 1-entry maps.
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        match self.0 {
            ron::Value::String(s) => visitor.visit_enum(s.as_str().into_deserializer()),
            ron::Value::Map(map) if map.len() == 1 => {
                let (key, value) = map.iter().next().expect("len checked");
                let variant = match key {
                    ron::Value::String(s) => s.clone(),
                    other => format!("{other:?}"),
                };
                visitor.visit_enum(FlexEnum { variant, value })
            }
            _ => self.deserialize_any(visitor),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map identifier ignored_any
    }
}

struct FlexEnum<'a> {
    variant: String,
    value: &'a ron::Value,
}

impl<'de> serde::de::EnumAccess<'de> for FlexEnum<'_> {
    type Error = DeError;
    type Variant = Self;
    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), DeError> {
        let variant = seed.deserialize(self.variant.clone().into_deserializer())?;
        Ok((variant, self))
    }
}

impl<'de> serde::de::VariantAccess<'de> for FlexEnum<'_> {
    type Error = DeError;
    fn unit_variant(self) -> Result<(), DeError> {
        Ok(())
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, DeError> {
        seed.deserialize(FlexValue(self.value))
    }
    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, DeError> {
        FlexValue(self.value).deserialize_any(visitor)
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        FlexValue(self.value).deserialize_any(visitor)
    }
}
