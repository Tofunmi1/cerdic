//! Shared provider boundary, provider composition, and named provider registry.
use crate::{Request, Response, ScriptedProvider};
use std::collections::BTreeMap;
use thiserror::Error;

/// A synchronous plug-in that produces a raw mind response for a request.
pub trait Provider {
    /// Responds to a request; the response id must match the request id.
    fn respond(&mut self, request: &Request) -> Response;

    /// Returns a short stable provider name.
    #[expect(
        clippy::unnecessary_literal_bound,
        reason = "the contract fixes a borrowed name return"
    )]
    fn name(&self) -> &str {
        "provider"
    }
}

impl<P: Provider + ?Sized> Provider for Box<P> {
    fn respond(&mut self, request: &Request) -> Response {
        (**self).respond(request)
    }
    fn name(&self) -> &str {
        (**self).name()
    }
}

impl Provider for ScriptedProvider {
    fn respond(&mut self, request: &Request) -> Response {
        ScriptedProvider::respond(self, request)
    }
    #[expect(
        clippy::unnecessary_literal_bound,
        reason = "provider names follow the shared borrowed API"
    )]
    fn name(&self) -> &str {
        "scripted"
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Provider for crate::CommandProvider {
    fn respond(&mut self, request: &Request) -> Response {
        crate::CommandProvider::respond(self, request)
    }
    #[expect(
        clippy::unnecessary_literal_bound,
        reason = "provider names follow the shared borrowed API"
    )]
    fn name(&self) -> &str {
        "command"
    }
}

/// Error returned when selecting an unknown registered provider.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ProviderError {
    /// The requested name is not present in the registry.
    #[error("unknown provider: {0}")]
    Unknown(String),
}

/// Holds named providers and the name used for unrouted requests.
#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<String, Box<dyn Provider>>,
    default: Option<String>,
}

impl ProviderRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers or replaces a provider; the first name becomes the default.
    pub fn register(&mut self, name: &str, provider: Box<dyn Provider>) {
        self.providers.insert(name.into(), provider);
        if self.default.is_none() {
            self.default = Some(name.into());
        }
    }

    /// Selects the default provider name.
    ///
    /// # Errors
    /// Returns [`ProviderError::Unknown`] if the name is not registered.
    pub fn set_default(&mut self, name: &str) -> Result<(), ProviderError> {
        if !self.providers.contains_key(name) {
            return Err(ProviderError::Unknown(name.into()));
        }
        self.default = Some(name.into());
        Ok(())
    }

    /// Returns the selected default name.
    #[must_use]
    pub fn default_name(&self) -> Option<&str> {
        self.default.as_deref()
    }

    /// Returns the selected default provider, if any.
    pub fn default_provider(&mut self) -> Option<&mut (dyn Provider + 'static)> {
        let name = self.default.as_deref()?;
        match self.providers.get_mut(name) {
            Some(provider) => Some(provider.as_mut()),
            None => None,
        }
    }

    /// Returns a named provider, if registered.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut (dyn Provider + 'static)> {
        self.providers.get_mut(name).map(Box::as_mut)
    }

    /// Returns registered names in sorted order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
}

/// Tries providers in order, stopping at the first successful response.
#[derive(Default)]
pub struct Chain {
    providers: Vec<Box<dyn Provider>>,
}

impl Chain {
    /// Creates an empty provider chain.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a provider to the end of the chain.
    #[must_use]
    pub fn then(mut self, provider: Box<dyn Provider>) -> Self {
        self.providers.push(provider);
        self
    }
}

impl Provider for Chain {
    fn respond(&mut self, request: &Request) -> Response {
        let mut failures = Vec::new();
        for provider in &mut self.providers {
            let name = provider.name().to_owned();
            let response = provider.respond(request);
            match response.outcome {
                Ok(_) => {
                    return Response {
                        id: request.id,
                        outcome: response.outcome,
                    };
                }
                Err(error) => failures.push(format!("{name}: {error}")),
            }
        }
        Response {
            id: request.id,
            outcome: Err(if failures.is_empty() {
                "no providers".into()
            } else {
                failures.join("; ")
            }),
        }
    }
    #[expect(
        clippy::unnecessary_literal_bound,
        reason = "provider names follow the shared borrowed API"
    )]
    fn name(&self) -> &str {
        "chain"
    }
}

/// Counts provider calls, failures, and Unicode character totals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeterStats {
    /// Number of requests handled.
    pub calls: u64,
    /// Number of failed responses.
    pub failures: u64,
    /// Total prompt characters.
    pub prompt_chars: usize,
    /// Successful reply characters.
    pub reply_chars: usize,
}

/// A provider wrapper that accumulates usage statistics.
pub struct Metered<P: Provider> {
    inner: P,
    stats: MeterStats,
}

impl<P: Provider> Metered<P> {
    /// Wraps a provider with zeroed counters.
    #[must_use]
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            stats: MeterStats::default(),
        }
    }

    /// Returns a copy of current usage statistics.
    #[must_use]
    pub fn stats(&self) -> MeterStats {
        self.stats
    }
}

impl<P: Provider> Provider for Metered<P> {
    fn respond(&mut self, request: &Request) -> Response {
        self.stats.calls = self.stats.calls.saturating_add(1);
        self.stats.prompt_chars = self
            .stats
            .prompt_chars
            .saturating_add(request.prompt.chars().count());
        let response = self.inner.respond(request);
        match &response.outcome {
            Ok(reply) => {
                self.stats.reply_chars =
                    self.stats.reply_chars.saturating_add(reply.chars().count());
            }
            Err(_) => self.stats.failures = self.stats.failures.saturating_add(1),
        }
        response
    }
    fn name(&self) -> &str {
        self.inner.name()
    }
}
