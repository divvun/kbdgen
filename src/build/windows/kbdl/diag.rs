/// Collects the warnings raised while generating one layout. Every warning
/// is logged through `tracing` with the layout's name as it is raised, and
/// kept so callers can inspect what was reported.
#[derive(Debug, Clone)]
pub struct Diagnostics {
    layout: String,
    warnings: Vec<String>,
}

impl Diagnostics {
    pub fn new(layout: impl Into<String>) -> Self {
        Diagnostics {
            layout: layout.into(),
            warnings: Vec::new(),
        }
    }

    pub fn layout(&self) -> &str {
        &self.layout
    }

    pub fn warn(&mut self, message: impl Into<String>) {
        let message = message.into();
        tracing::warn!("{}: {}", self.layout, message);
        self.warnings.push(message);
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
}
