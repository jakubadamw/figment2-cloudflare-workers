//! A [figment2] provider for Cloudflare Worker environment bindings.
//!
//! This crate bridges [figment2]'s configuration system with the
//! [Cloudflare Workers](https://workers.cloudflare.com/) runtime, reading
//! values from [`worker::Env`] bindings.
//!
//! # Usage
//!
//! Construct a [`CloudflareWorkersBindings`] provider via
//! [`from_struct`](CloudflareWorkersBindings::from_struct), passing the target
//! configuration type as a type parameter. The provider uppercases field names
//! to derive the Cloudflare binding names, and reads each one from [`worker::Env`]:
//!
//! ```rust,ignore
//! use figment2::Figment;
//! use figment2_cloudflare_env::CloudflareWorkersBindings;
//! use serde::Deserialize;
//!
//! #[derive(Deserialize)]
//! struct Config {
//!     database_url: String,
//!     max_connections: u16,
//! }
//!
//! let config: Config = Figment::new()
//!     .merge(CloudflareWorkersBindings::from_struct::<Config>(&env))
//!     .extract()
//!     .expect("valid configuration");
//! ```
//!
//! The above looks up `DATABASE_URL` and `MAX_CONNECTIONS` in the worker
//! environment automatically.
//!
//! # Vars vs. secrets
//!
//! Cloudflare Workers distinguish between plain-text **variables** and
//! **secrets**, but a given binding name can only be one or the other. The
//! provider therefore tries [`worker::Env::var`] first and falls back to
//! [`worker::Env::secret`] — no manual annotation is needed.
//!
//! At the struct level, the recommended convention is to use
//! [`secrecy::SecretString`] for fields backed by secrets. This prevents
//! accidental logging and ensures the value is zeroised on drop, making the
//! distinction visible in the type system:
//!
//! ```rust,ignore
//! use secrecy::SecretString;
//!
//! #[derive(Deserialize)]
//! struct Config {
//!     database_url: String,           // Cloudflare var
//!     api_key: SecretString,          // Cloudflare secret
//! }
//! ```

use figment2::{
    value::{Dict, Map, Value},
    Error, Metadata, Profile, Provider,
};
use serde::de::{self, DeserializeOwned, Deserializer, Visitor};

/// A [figment2] provider that reads values from a Cloudflare Worker
/// environment.
///
/// Field names are discovered from the target struct's [`Deserialize`]
/// implementation and uppercased to derive Cloudflare binding names
/// (e.g. `database_url` → `DATABASE_URL`). For each binding,
/// [`worker::Env::var`] is tried first; if that fails,
/// [`worker::Env::secret`] is used as a fallback.
///
/// Missing bindings are silently skipped, allowing other providers in the
/// [figment2] stack to supply defaults.
pub struct CloudflareWorkersBindings<'a> {
    env: &'a worker::Env,
    fields: Vec<String>,
    profile: Profile,
}

impl<'a> CloudflareWorkersBindings<'a> {
    /// Create a provider that reads all fields declared in `T` from the
    /// Cloudflare Worker environment.
    #[must_use]
    pub fn from_struct<T: DeserializeOwned>(env: &'a worker::Env) -> Self {
        Self {
            env,
            fields: field_names::<T>(),
            profile: Profile::Default,
        }
    }

    /// Set the [figment2] profile to emit values into.
    #[must_use]
    pub fn profile(mut self, profile: impl Into<Profile>) -> Self {
        self.profile = profile.into();
        self
    }
}

impl Provider for CloudflareWorkersBindings<'_> {
    fn metadata(&self) -> Metadata {
        Metadata::named("Cloudflare Worker environment")
    }

    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        let dict = self
            .fields
            .iter()
            .filter_map(|field| {
                let binding = field.to_uppercase();
                let value = self
                    .env
                    .var(&binding)
                    .map(|var| var.to_string())
                    .ok()
                    .or_else(|| {
                        self.env
                            .secret(&binding)
                            .map(|secret| secret.to_string())
                            .ok()
                    })?;
                Some((field.clone(), Value::from(value)))
            })
            .collect::<Dict>();

        Ok(self.profile.collect(dict))
    }
}

/// Discover the field names of a `#[derive(Deserialize)]` struct by running
/// a dummy deserialisation that captures the `fields` slice passed to
/// [`Deserializer::deserialize_struct`].
fn field_names<T: DeserializeOwned>() -> Vec<String> {
    struct Extractor(Vec<String>);

    impl<'de> Deserializer<'de> for &mut Extractor {
        type Error = de::value::Error;

        fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
            Err(de::Error::custom("field extraction only"))
        }

        fn deserialize_struct<V: Visitor<'de>>(
            self,
            _name: &'static str,
            fields: &'static [&'static str],
            _visitor: V,
        ) -> Result<V::Value, Self::Error> {
            self.0 = fields.iter().map(|field| (*field).to_owned()).collect();
            Err(de::Error::custom("field extraction only"))
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
            bytes byte_buf option unit unit_struct newtype_struct seq tuple
            tuple_struct map enum identifier ignored_any
        }
    }

    let mut extractor = Extractor(Vec::new());
    let _ = T::deserialize(&mut extractor);
    extractor.0
}
