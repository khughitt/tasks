use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

pub(super) const MAX_INPUT: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Harness {
    Claude,
    Codex,
    Agy,
}

impl Harness {
    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude-code",
            Self::Codex => "codex",
            Self::Agy => "agy",
        }
    }
}

pub(super) struct HookInput {
    pub harness: Harness,
    pub event: Value,
}

/// Decode all members, including unknown ones, before any map can overwrite a key.
struct Strict(Value);
impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Json;
        impl<'de> Visitor<'de> for Json {
            type Value = Strict;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("strict JSON")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Strict, E> {
                Number::from_f64(v)
                    .map(|n| Strict(n.into()))
                    .ok_or_else(|| E::custom("invalid number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Strict, E> {
                Ok(Strict(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Strict, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Strict, A::Error> {
                let mut list = Vec::new();
                while let Some(Strict(value)) = seq.next_element()? {
                    list.push(value);
                }
                Ok(Strict(list.into()))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Strict, A::Error> {
                let mut object = Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if object.contains_key(&key) {
                        return Err(de::Error::custom("repeated object key"));
                    }
                    let Strict(value) = map.next_value()?;
                    object.insert(key, value);
                }
                Ok(Strict(object.into()))
            }
        }
        deserializer.deserialize_any(Json)
    }
}

pub(super) fn strict_json(bytes: &[u8]) -> Result<Value, String> {
    std::str::from_utf8(bytes).map_err(|_| "input is not UTF-8")?;
    // Do not include serde's error text: it can quote private input.
    serde_json::from_slice::<Strict>(bytes)
        .map(|v| v.0)
        .map_err(|_| "invalid strict JSON".into())
}

pub(super) fn parse(bytes: &[u8]) -> Result<HookInput, String> {
    if bytes.len() > MAX_INPUT {
        return Err("input exceeds 4 MiB".into());
    }
    let doc = strict_json(bytes)?;
    if doc["schema"].as_u64() != Some(2)
        || doc["type"] != "event"
        || doc["event"]["schema"].as_u64() != Some(2)
    {
        return Err("expected a schema-2 SubscriberEnvelope event".into());
    }
    let harness = match doc["event"]["harness"].as_str() {
        Some("claude-code") => Harness::Claude,
        Some("codex") => Harness::Codex,
        Some("agy") => Harness::Agy,
        _ => return Err("unsupported event harness".into()),
    };
    Ok(HookInput {
        harness,
        event: doc["event"].clone(),
    })
}

pub(super) enum Fact<'a> {
    Missing,
    Invalid,
    Valid(&'a Value),
}
pub(super) fn fact<'a>(container: &'a Value, key: &str) -> Fact<'a> {
    match container.get(key) {
        None => Fact::Missing,
        Some(v) => match v["status"].as_str() {
            Some("missing") => Fact::Missing,
            Some("valid") if v.get("value").is_some() => Fact::Valid(&v["value"]),
            _ => Fact::Invalid,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_input_rejects_nested_duplicate_keys() {
        for bytes in [
            br#"{"unknown":{"x":1,"x":2}}"#.as_slice(),
            br#"{"a":[{"x":1,"x":2}]}"#,
        ] {
            assert!(strict_json(bytes).is_err());
        }
        for bytes in [b"{} {}".as_slice(), b"{\"x\":\"\xff\"}", b"[NaN]"] {
            assert!(strict_json(bytes).is_err());
        }
        assert!(strict_json(br#"{"unknown":[null,true,1,1.0,"text"]}"#).is_ok());
        assert!(parse(&vec![b' '; MAX_INPUT + 1]).is_err());
    }
}
