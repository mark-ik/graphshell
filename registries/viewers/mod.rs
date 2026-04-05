mod plaintext;
mod image_viewer;
mod directory;
#[cfg(feature = "pdf")]
mod pdf;
#[cfg(feature = "audio")]
mod audio;

pub(crate) use plaintext::PlaintextEmbeddedViewer;
pub(crate) use image_viewer::ImageEmbeddedViewer;
pub(crate) use directory::DirectoryEmbeddedViewer;
#[cfg(feature = "pdf")]
pub(crate) use pdf::PdfEmbeddedViewer;
#[cfg(feature = "audio")]
pub(crate) use audio::AudioEmbeddedViewer;
