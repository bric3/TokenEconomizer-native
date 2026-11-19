use crate::de::ParserNumber;
use crate::error::Error;
#[cfg(feature = "arbitrary_precision")]
use crate::error::ErrorCode;
#[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
use alloc::borrow::ToOwned;
#[cfg(feature = "python_numbers")]
use alloc::boxed::Box;
#[cfg(feature = "arbitrary_precision")]
use alloc::string::{String, ToString};
use core::fmt::{self, Debug, Display};
#[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
use core::hash::{Hash, Hasher};
use serde::de::{self, Unexpected, Visitor};
#[cfg(feature = "arbitrary_precision")]
use serde::de::{IntoDeserializer, MapAccess};
use serde::{forward_to_deserialize_any, Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "arbitrary_precision")]
pub(crate) const TOKEN: &str = "$serde_json::private::Number";

/// Represents a JSON number, whether integer or floating point.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Number {
    n: N,
}

#[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
#[derive(Clone)]
#[cfg_attr(not(feature = "python_numbers"), derive(Copy))]
enum N {
    PosInt(u64),
    /// Always less than zero.
    NegInt(i64),
    /// Finite unless python_numbers is enabled.
    Float(f64),
    #[cfg(feature = "python_numbers")]
    BigInt(Box<str>),
}

#[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
impl PartialEq for N {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (N::PosInt(a), N::PosInt(b)) => a == b,
            (N::NegInt(a), N::NegInt(b)) => a == b,
            (N::Float(a), N::Float(b)) => {
                a == b || cfg!(feature = "python_numbers") && a.is_nan() && b.is_nan()
            }
            #[cfg(feature = "python_numbers")]
            (N::BigInt(a), N::BigInt(b)) => a == b,
            _ => false,
        }
    }
}

// NaNs compare equal as JSON values; floating-point comparisons retain IEEE semantics.
#[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
impl Eq for N {}

#[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
impl Hash for N {
    fn hash<H: Hasher>(&self, h: &mut H) {
        match *self {
            N::PosInt(i) => i.hash(h),
            N::NegInt(i) => i.hash(h),
            #[cfg(feature = "python_numbers")]
            N::BigInt(ref value) => value.hash(h),
            N::Float(f) => {
                if cfg!(feature = "python_numbers") && f.is_nan() {
                    f64::NAN.to_bits().hash(h);
                } else if f == 0.0f64 {
                    // There are 2 zero representations, +0 and -0, which
                    // compare equal but have different bits. We use the +0 hash
                    // for both so that hash(+0) == hash(-0).
                    0.0f64.to_bits().hash(h);
                } else {
                    f.to_bits().hash(h);
                }
            }
        }
    }
}

#[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
type N = String;

#[cfg(feature = "python_numbers")]
pub(crate) fn nonfinite(value: f64) -> Option<&'static str> {
    if value.is_nan() {
        Some("NaN")
    } else if value == f64::INFINITY {
        Some("Infinity")
    } else if value == f64::NEG_INFINITY {
        Some("-Infinity")
    } else {
        None
    }
}

impl Number {
    /// Returns true if the `Number` is an integer between `i64::MIN` and
    /// `i64::MAX`.
    ///
    /// For any Number on which `is_i64` returns true, `as_i64` is guaranteed to
    /// return the integer value.
    pub fn is_i64(&self) -> bool {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        match self.n {
            N::PosInt(v) => v <= i64::MAX as u64,
            N::NegInt(_) => true,
            N::Float(_) => false,
            #[cfg(feature = "python_numbers")]
            N::BigInt(_) => false,
        }
        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        self.as_i64().is_some()
    }

    /// Returns true if the `Number` is an integer between zero and `u64::MAX`.
    ///
    /// For any Number on which `is_u64` returns true, `as_u64` is guaranteed to
    /// return the integer value.
    pub fn is_u64(&self) -> bool {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        match self.n {
            N::PosInt(_) => true,
            N::NegInt(_) | N::Float(_) => false,
            #[cfg(feature = "python_numbers")]
            N::BigInt(_) => false,
        }
        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        self.as_u64().is_some()
    }

    /// Returns true if the `Number` can be represented by f64.
    ///
    /// For any Number on which `is_f64` returns true, `as_f64` is guaranteed to
    /// return the floating point value.
    ///
    /// Currently this function returns true if and only if both `is_i64` and
    /// `is_u64` return false but this is not a guarantee in the future.
    pub fn is_f64(&self) -> bool {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        match self.n {
            N::Float(_) => true,
            N::PosInt(_) | N::NegInt(_) => false,
            #[cfg(feature = "python_numbers")]
            N::BigInt(_) => false,
        }
        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        {
            for c in self.n.chars() {
                if c == '.' || c == 'e' || c == 'E' {
                    return self.n.parse::<f64>().is_ok_and(f64::is_finite);
                }
            }
            false
        }
    }

    /// If the `Number` is an integer, represent it as i64 if possible. Returns
    /// None otherwise.
    pub fn as_i64(&self) -> Option<i64> {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        match self.n {
            #[cfg(feature = "python_numbers")]
            N::BigInt(_) => None,
            N::PosInt(n) => {
                if n <= i64::MAX as u64 {
                    Some(n as i64)
                } else {
                    None
                }
            }
            N::NegInt(n) => Some(n),
            N::Float(_) => None,
        }
        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        self.n.parse().ok()
    }

    /// If the `Number` is an integer, represent it as u64 if possible. Returns
    /// None otherwise.
    pub fn as_u64(&self) -> Option<u64> {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        match self.n {
            #[cfg(feature = "python_numbers")]
            N::BigInt(_) => None,
            N::PosInt(n) => Some(n),
            N::NegInt(_) | N::Float(_) => None,
        }
        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        self.n.parse().ok()
    }

    /// Represents the number as f64 if possible. Returns None otherwise.
    pub fn as_f64(&self) -> Option<f64> {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        match self.n {
            #[cfg(feature = "python_numbers")]
            N::BigInt(ref value) => value.parse::<f64>().ok().filter(|n| n.is_finite()),
            N::PosInt(n) => Some(n as f64),
            N::NegInt(n) => Some(n as f64),
            N::Float(n) => Some(n),
        }
        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        self.n.parse::<f64>().ok().filter(|float| float.is_finite())
    }

    /// Converts a finite `f64` to a `Number`. Infinite or NaN values are not JSON
    /// numbers.
    ///
    /// ```
    /// # use serde_json::Number;
    /// #
    /// assert!(Number::from_f64(256.0).is_some());
    ///
    /// assert!(Number::from_f64(f64::NAN).is_none());
    /// ```
    pub fn from_f64(f: f64) -> Option<Number> {
        if cfg!(feature = "python_numbers") || f.is_finite() {
            let n = {
                #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
                {
                    N::Float(f)
                }
                #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
                {
                    zmij::Buffer::new().format_finite(f).to_owned()
                }
            };
            Some(Number { n })
        } else {
            None
        }
    }

    /// If the `Number` is an integer, represent it as i128 if possible. Returns
    /// None otherwise.
    pub fn as_i128(&self) -> Option<i128> {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        match self.n {
            #[cfg(feature = "python_numbers")]
            N::BigInt(ref value) => value.parse().ok(),
            N::PosInt(n) => Some(n as i128),
            N::NegInt(n) => Some(n as i128),
            N::Float(_) => None,
        }
        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        self.n.parse().ok()
    }

    /// If the `Number` is an integer, represent it as u128 if possible. Returns
    /// None otherwise.
    pub fn as_u128(&self) -> Option<u128> {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        match self.n {
            #[cfg(feature = "python_numbers")]
            N::BigInt(ref value) => value.parse().ok(),
            N::PosInt(n) => Some(n as u128),
            N::NegInt(_) | N::Float(_) => None,
        }
        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        self.n.parse().ok()
    }

    /// Converts an `i128` to a `Number`. Numbers smaller than i64::MIN or
    /// larger than u64::MAX can only be represented in `Number` if serde_json's
    /// "arbitrary_precision" feature is enabled.
    ///
    /// ```
    /// # use serde_json::Number;
    /// #
    /// assert!(Number::from_i128(256).is_some());
    /// ```
    pub fn from_i128(i: i128) -> Option<Number> {
        let n = {
            #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
            {
                if let Ok(u) = u64::try_from(i) {
                    N::PosInt(u)
                } else if let Ok(i) = i64::try_from(i) {
                    N::NegInt(i)
                } else {
                    #[cfg(feature = "python_numbers")]
                    {
                        N::BigInt(i.to_string().into_boxed_str())
                    }
                    #[cfg(not(feature = "python_numbers"))]
                    {
                        return None;
                    }
                }
            }
            #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
            {
                i.to_string()
            }
        };
        Some(Number { n })
    }

    /// Converts a `u128` to a `Number`. Numbers greater than u64::MAX can only
    /// be represented in `Number` if serde_json's "arbitrary_precision" feature
    /// is enabled.
    ///
    /// ```
    /// # use serde_json::Number;
    /// #
    /// assert!(Number::from_u128(256).is_some());
    /// ```
    pub fn from_u128(i: u128) -> Option<Number> {
        let n = {
            #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
            {
                if let Ok(u) = u64::try_from(i) {
                    N::PosInt(u)
                } else {
                    #[cfg(feature = "python_numbers")]
                    {
                        N::BigInt(i.to_string().into_boxed_str())
                    }
                    #[cfg(not(feature = "python_numbers"))]
                    {
                        return None;
                    }
                }
            }
            #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
            {
                i.to_string()
            }
        };
        Some(Number { n })
    }

    /// Returns the JSON representation that this Number was parsed from.
    ///
    /// When parsing with serde_json's `arbitrary_precision` feature enabled,
    /// positive exponents are normalized to include an explicit `+` sign so
    /// that `1e140` becomes `1e+140`.
    ///
    /// For numbers constructed not via parsing, such as by `From<i32>`, returns
    /// the JSON representation that serde\_json would serialize for this
    /// number.
    ///
    /// ```
    /// # use serde_json::Number;
    /// for value in [
    ///     "7",
    ///     "12.34",
    ///     "34e-56789",
    ///     "0.0123456789000000012345678900000001234567890000123456789",
    ///     "343412345678910111213141516171819202122232425262728293034",
    ///     "-343412345678910111213141516171819202122232425262728293031",
    /// ] {
    ///     let number: Number = serde_json::from_str(value).unwrap();
    ///     assert_eq!(number.as_str(), value);
    /// }
    /// ```
    #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "arbitrary_precision")))]
    pub fn as_str(&self) -> &str {
        &self.n
    }

    pub(crate) fn as_f32(&self) -> Option<f32> {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        match self.n {
            #[cfg(feature = "python_numbers")]
            N::BigInt(ref value) => value.parse::<f32>().ok().filter(|n| n.is_finite()),
            N::PosInt(n) => Some(n as f32),
            N::NegInt(n) => Some(n as f32),
            N::Float(n) => Some(n as f32),
        }
        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        self.n.parse::<f32>().ok().filter(|float| float.is_finite())
    }

    pub(crate) fn from_f32(f: f32) -> Option<Number> {
        if cfg!(feature = "python_numbers") || f.is_finite() {
            let n = {
                #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
                {
                    N::Float(f as f64)
                }
                #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
                {
                    zmij::Buffer::new().format_finite(f).to_owned()
                }
            };
            Some(Number { n })
        } else {
            None
        }
    }

    #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
    /// Not public API. Only tests use this.
    #[doc(hidden)]
    #[inline]
    pub fn from_string_unchecked(n: String) -> Self {
        Number { n }
    }
}

impl Display for Number {
    #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self.n {
            N::PosInt(u) => formatter.write_str(itoa::Buffer::new().format(u)),
            N::NegInt(i) => formatter.write_str(itoa::Buffer::new().format(i)),
            N::Float(f) => {
                #[cfg(feature = "python_numbers")]
                if let Some(value) = nonfinite(f) {
                    return formatter.write_str(value);
                }
                formatter.write_str(zmij::Buffer::new().format_finite(f))
            }
            #[cfg(feature = "python_numbers")]
            N::BigInt(ref value) => formatter.write_str(value),
        }
    }

    #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.n, formatter)
    }
}

impl Debug for Number {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "Number({})", self)
    }
}

impl Serialize for Number {
    #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.n {
            N::PosInt(u) => serializer.serialize_u64(u),
            N::NegInt(i) => serializer.serialize_i64(i),
            N::Float(f) => serializer.serialize_f64(f),
            #[cfg(feature = "python_numbers")]
            N::BigInt(ref value) => {
                use serde::ser::SerializeStruct;
                let mut output = tri!(serializer.serialize_struct(TOKEN, 1));
                tri!(output.serialize_field(TOKEN, value));
                output.end()
            }
        }
    }

    #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut s = tri!(serializer.serialize_struct(TOKEN, 1));
        tri!(s.serialize_field(TOKEN, &self.n));
        s.end()
    }
}

impl<'de> Deserialize<'de> for Number {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Number, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NumberVisitor;

        impl<'de> Visitor<'de> for NumberVisitor {
            type Value = Number;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON number")
            }

            fn visit_i64<E>(self, value: i64) -> Result<Number, E> {
                Ok(value.into())
            }

            fn visit_i128<E>(self, value: i128) -> Result<Number, E>
            where
                E: de::Error,
            {
                Number::from_i128(value)
                    .ok_or_else(|| de::Error::custom("JSON number out of range"))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Number, E> {
                Ok(value.into())
            }

            fn visit_u128<E>(self, value: u128) -> Result<Number, E>
            where
                E: de::Error,
            {
                Number::from_u128(value)
                    .ok_or_else(|| de::Error::custom("JSON number out of range"))
            }

            fn visit_f64<E>(self, value: f64) -> Result<Number, E>
            where
                E: de::Error,
            {
                Number::from_f64(value).ok_or_else(|| de::Error::custom("not a JSON number"))
            }

            #[cfg(feature = "arbitrary_precision")]
            fn visit_map<V>(self, mut visitor: V) -> Result<Number, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let value = tri!(visitor.next_key::<NumberKey>());
                if value.is_none() {
                    return Err(de::Error::invalid_type(Unexpected::Map, &self));
                }
                let v: NumberFromString = tri!(visitor.next_value());
                Ok(v.value)
            }
        }

        deserializer.deserialize_any(NumberVisitor)
    }
}

#[cfg(feature = "arbitrary_precision")]
struct NumberKey;

#[cfg(feature = "arbitrary_precision")]
impl<'de> de::Deserialize<'de> for NumberKey {
    fn deserialize<D>(deserializer: D) -> Result<NumberKey, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct FieldVisitor;

        impl<'de> de::Visitor<'de> for FieldVisitor {
            type Value = ();

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid number field")
            }

            fn visit_str<E>(self, s: &str) -> Result<(), E>
            where
                E: de::Error,
            {
                if s == TOKEN {
                    Ok(())
                } else {
                    Err(de::Error::custom("expected field with custom name"))
                }
            }
            #[cfg(feature = "python_numbers")]
            fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<(), E> {
                if bytes == TOKEN.as_bytes() {
                    Ok(())
                } else {
                    Err(de::Error::custom("invalid internal number field"))
                }
            }
        }

        tri!(deserializer.deserialize_identifier(FieldVisitor));
        Ok(NumberKey)
    }
}

#[cfg(feature = "arbitrary_precision")]
pub struct NumberFromString {
    pub value: Number,
}

#[cfg(feature = "arbitrary_precision")]
impl<'de> de::Deserialize<'de> for NumberFromString {
    fn deserialize<D>(deserializer: D) -> Result<NumberFromString, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = NumberFromString;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string containing a number")
            }

            fn visit_str<E>(self, s: &str) -> Result<NumberFromString, E>
            where
                E: de::Error,
            {
                let n = tri!(s.parse().map_err(de::Error::custom));
                Ok(NumberFromString { value: n })
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

#[cfg(feature = "arbitrary_precision")]
fn invalid_number() -> Error {
    Error::syntax(ErrorCode::InvalidNumber, 0, 0)
}

macro_rules! deserialize_any {
    (@expand [$($num_string:tt)*]) => {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.n {
                N::PosInt(u) => visitor.visit_u64(u),
                N::NegInt(i) => visitor.visit_i64(i),
                N::Float(f) => visitor.visit_f64(f),
                #[cfg(feature = "python_numbers")]
                N::BigInt(ref value) => visitor.visit_map(NumberDeserializer { number: Some(value.to_string()) }),
            }
        }

        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
            where V: Visitor<'de>
        {
            if let Some(u) = self.as_u64() {
                return visitor.visit_u64(u);
            } else if let Some(i) = self.as_i64() {
                return visitor.visit_i64(i);
            } else if let Some(u) = self.as_u128() {
                return visitor.visit_u128(u);
            } else if let Some(i) = self.as_i128() {
                return visitor.visit_i128(i);
            } else if let Some(f) = self.as_f64() {
                if zmij::Buffer::new().format_finite(f) == self.n || f.to_string() == self.n {
                    return visitor.visit_f64(f);
                }
            }

            visitor.visit_map(NumberDeserializer {
                number: Some(self.$($num_string)*),
            })
        }
    };

    (owned) => {
        deserialize_any!(@expand [n]);
    };

    (ref) => {
        deserialize_any!(@expand [n.clone()]);
    };
}

macro_rules! deserialize_number {
    ($deserialize:ident => $visit:ident) => {
        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
        fn $deserialize<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            #[cfg(feature = "python_numbers")]
            if let N::BigInt(ref value) = self.n {
                return visitor.$visit(tri!(value.parse().map_err(|_| invalid_number())));
            }
            self.deserialize_any(visitor)
        }

        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
        fn $deserialize<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: de::Visitor<'de>,
        {
            visitor.$visit(tri!(self.n.parse().map_err(|_| invalid_number())))
        }
    };
}

impl<'de> Deserializer<'de> for Number {
    type Error = Error;

    deserialize_any!(owned);

    deserialize_number!(deserialize_i8 => visit_i8);
    deserialize_number!(deserialize_i16 => visit_i16);
    deserialize_number!(deserialize_i32 => visit_i32);
    deserialize_number!(deserialize_i64 => visit_i64);
    deserialize_number!(deserialize_i128 => visit_i128);
    deserialize_number!(deserialize_u8 => visit_u8);
    deserialize_number!(deserialize_u16 => visit_u16);
    deserialize_number!(deserialize_u32 => visit_u32);
    deserialize_number!(deserialize_u64 => visit_u64);
    deserialize_number!(deserialize_u128 => visit_u128);
    deserialize_number!(deserialize_f32 => visit_f32);
    deserialize_number!(deserialize_f64 => visit_f64);

    forward_to_deserialize_any! {
        bool char str string bytes byte_buf option unit unit_struct
        newtype_struct seq tuple tuple_struct map struct enum identifier
        ignored_any
    }
}

impl<'de> Deserializer<'de> for &Number {
    type Error = Error;

    deserialize_any!(ref);

    deserialize_number!(deserialize_i8 => visit_i8);
    deserialize_number!(deserialize_i16 => visit_i16);
    deserialize_number!(deserialize_i32 => visit_i32);
    deserialize_number!(deserialize_i64 => visit_i64);
    deserialize_number!(deserialize_i128 => visit_i128);
    deserialize_number!(deserialize_u8 => visit_u8);
    deserialize_number!(deserialize_u16 => visit_u16);
    deserialize_number!(deserialize_u32 => visit_u32);
    deserialize_number!(deserialize_u64 => visit_u64);
    deserialize_number!(deserialize_u128 => visit_u128);
    deserialize_number!(deserialize_f32 => visit_f32);
    deserialize_number!(deserialize_f64 => visit_f64);

    forward_to_deserialize_any! {
        bool char str string bytes byte_buf option unit unit_struct
        newtype_struct seq tuple tuple_struct map struct enum identifier
        ignored_any
    }
}

#[cfg(feature = "arbitrary_precision")]
pub(crate) struct NumberDeserializer {
    pub number: Option<String>,
}

#[cfg(feature = "arbitrary_precision")]
impl<'de> MapAccess<'de> for NumberDeserializer {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        if self.number.is_none() {
            return Ok(None);
        }
        seed.deserialize(NumberFieldDeserializer).map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        seed.deserialize(self.number.take().unwrap().into_deserializer())
    }
}

#[cfg(feature = "arbitrary_precision")]
struct NumberFieldDeserializer;

#[cfg(feature = "arbitrary_precision")]
impl<'de> Deserializer<'de> for NumberFieldDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: de::Visitor<'de>,
    {
        #[cfg(feature = "python_numbers")]
        {
            visitor.visit_borrowed_bytes(TOKEN.as_bytes())
        }
        #[cfg(not(feature = "python_numbers"))]
        {
            visitor.visit_borrowed_str(TOKEN)
        }
    }

    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 char str string seq
        bytes byte_buf map struct option unit newtype_struct ignored_any
        unit_struct tuple_struct tuple enum identifier
    }
}

impl From<ParserNumber> for Number {
    fn from(value: ParserNumber) -> Self {
        let n = match value {
            ParserNumber::F64(f) => {
                #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
                {
                    N::Float(f)
                }
                #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
                {
                    zmij::Buffer::new().format_finite(f).to_owned()
                }
            }
            ParserNumber::U64(u) => {
                #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
                {
                    N::PosInt(u)
                }
                #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
                {
                    itoa::Buffer::new().format(u).to_owned()
                }
            }
            ParserNumber::I64(i) => {
                #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
                {
                    N::NegInt(i)
                }
                #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
                {
                    itoa::Buffer::new().format(i).to_owned()
                }
            }
            #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
            ParserNumber::String(s) => s,
            #[cfg(feature = "python_numbers")]
            ParserNumber::String(s) => N::BigInt(s.into_boxed_str()),
        };
        Number { n }
    }
}

macro_rules! impl_from_unsigned {
    (
        $($ty:ty),*
    ) => {
        $(
            impl From<$ty> for Number {
                fn from(u: $ty) -> Self {
                    let n = {
                        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
                        { N::PosInt(u as u64) }
                        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
                        {
                            itoa::Buffer::new().format(u).to_owned()
                        }
                    };
                    Number { n }
                }
            }
        )*
    };
}

macro_rules! impl_from_signed {
    (
        $($ty:ty),*
    ) => {
        $(
            impl From<$ty> for Number {
                fn from(i: $ty) -> Self {
                    let n = {
                        #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
                        {
                            if i < 0 {
                                N::NegInt(i as i64)
                            } else {
                                N::PosInt(i as u64)
                            }
                        }
                        #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
                        {
                            itoa::Buffer::new().format(i).to_owned()
                        }
                    };
                    Number { n }
                }
            }
        )*
    };
}

impl_from_unsigned!(u8, u16, u32, u64, usize);
impl_from_signed!(i8, i16, i32, i64, isize);

#[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
impl_from_unsigned!(u128);
#[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
impl_from_signed!(i128);

#[cfg(feature = "python_numbers")]
impl From<u128> for Number {
    fn from(value: u128) -> Self {
        Self::from_u128(value).unwrap()
    }
}
#[cfg(feature = "python_numbers")]
impl From<i128> for Number {
    fn from(value: i128) -> Self {
        Self::from_i128(value).unwrap()
    }
}

impl Number {
    #[cfg(any(not(feature = "arbitrary_precision"), feature = "python_numbers"))]
    #[cold]
    pub(crate) fn unexpected(&self) -> Unexpected {
        match self.n {
            N::PosInt(u) => Unexpected::Unsigned(u),
            N::NegInt(i) => Unexpected::Signed(i),
            N::Float(f) => Unexpected::Float(f),
            #[cfg(feature = "python_numbers")]
            N::BigInt(_) => Unexpected::Other("number"),
        }
    }

    #[cfg(all(feature = "arbitrary_precision", not(feature = "python_numbers")))]
    #[cold]
    pub(crate) fn unexpected(&self) -> Unexpected {
        Unexpected::Other("number")
    }
}
