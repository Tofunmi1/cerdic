//! The provider boundary. A provider (a model, or the offline mock) sees the document and a request
//! and returns one bounded typed patch; the host applies it through the same commands as every
//! other edit, with one capped repair attempt. A provider never gets to run code and never holds
//! the document.

mod assist;
mod mock;
mod patch;

pub use assist::{Assisted, Authoring, assist};
pub use mock::{MockProvider, VASE_TEXT};
pub use patch::Patch;
use ve_graph::{Diagnostic, Document};

/// What the provider is shown.
#[derive(Clone, Debug)]
pub struct Request {
    /// The user's words.
    pub prompt: String,
    /// The document as it is now.
    pub document: Document,
    /// Its current revision.
    pub revision: u64,
    /// Why an earlier attempt failed, on the single repair attempt.
    pub feedback: Vec<Diagnostic>,
}

/// Why a provider gave no usable reply.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    /// The provider does not handle this request.
    #[error("unsupported request: {0}")]
    Unsupported(String),
    /// The provider failed.
    #[error("provider failed: {0}")]
    Failed(String),
}

/// A source of patches.
pub trait Provider {
    /// A name that says what it is (mocks say so).
    fn name(&self) -> &'static str;

    /// Returns the raw reply text; the host validates it as a [`Patch`].
    ///
    /// # Errors
    /// [`ProviderError`] when there is no reply.
    fn propose(&mut self, request: &Request) -> Result<String, ProviderError>;
}
