#![warn(missing_docs)]
/*!
A more lightweight alternative to [color-eyre], which simply grants access
to the span where an error happened, allowing them to be printed into standard logging facilityies.

To use, [`install`] the handler, after which you can get the span with [`ReportSpan::span`]
or immediately log a `Result` with [`emit`] or its method alias [`Emit::emit`].

This may not work correctly with all subscriber, but it works fine with the standard `tracing_subscriber::fmt`.

If the `tracing-error` feature is enabled (default), the `Display` implementation will show a span trace.

[color-eyre]: https://docs.rs/color-eyre/latest/color_eyre/
*/

use eyre::Report;
use tracing::Span;

#[derive(Debug)]
struct Handler {
	span: Span,
}

impl eyre::EyreHandler for Handler {
	fn debug(&self, error: &dyn std::error::Error, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		std::fmt::Debug::fmt(error, f)
	}

	#[cfg(feature = "tracing-error")]
	fn display(&self, e: &dyn std::error::Error, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		use std::fmt::Write;

		std::fmt::Display::fmt(e, f)?;

		if f.alternate() {
			let mut s = String::new();
			tracing_error::SpanTrace::new(self.span.clone())
				.with_spans(|meta, fields| {
					write!(s, "\n• {}::{}", meta.target(), meta.name()).unwrap();
					if !fields.is_empty() {
						write!(s, "{{{}}}", strip_ansi(fields.to_owned())).unwrap();
					}
					true
				});
			f.write_str(&s)?;
		}
		Ok(())
	}
}

fn strip_ansi(mut s: String) -> String {
	let mut keep = true;
	s.retain(|c| match c {
		'\x1B' => { keep = false; false }
		'm' if !keep => { keep = true; false }
		_ => keep
	});
	s
}

mod seal {
	pub trait Sealed {}
}

impl<T> seal::Sealed for eyre::Result<T> {}
impl seal::Sealed for Report {}

/// Extension trait for the [`span`](ReportSpan::span) method.
pub trait ReportSpan: seal::Sealed {
	/// Returns the span the error occurred in.
	///
	/// Panics if the handler was not installed.
	fn span(&self) -> &Span;
}

impl ReportSpan for Report {
	fn span(&self) -> &Span {
		&self.handler()
			.downcast_ref::<Handler>()
			.expect("eyre-span handler")
			.span
	}
}

/// Extension trait for the [`emit`](Emit::emit) method.
pub trait Emit<T>: seal::Sealed {
	/// Method syntax for [`emit`].
	fn emit(self) -> Option<T>;
}

impl<T> Emit<T> for Result<T, Report> {
	fn emit(self) -> Option<T> {
		emit(self)
	}
}

/// Sends a [`tracing::error!`] event if an error happened.
///
/// Panics if the handler was not installed.
pub fn emit<T>(e: Result<T, Report>) -> Option<T> {
	match e {
		Ok(v) => Some(v),
		Err(e) => {
			e.span().in_scope(|| tracing::error!("{e}"));
			None
		}
	}
}

/// Installs the hook into Eyre. Required for this crate to function.
pub fn install() -> Result<(), eyre::InstallError> {
	eyre::set_hook(Box::new(|_| Box::new(Handler { span: tracing::Span::current() })))
}
