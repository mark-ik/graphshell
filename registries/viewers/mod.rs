#[cfg(feature = "audio")]
mod audio;
mod directory;
mod image_viewer;
mod middlenet;
#[cfg(feature = "pdf")]
mod pdf;
mod plaintext;

#[cfg(feature = "audio")]
pub(crate) use audio::AudioEmbeddedViewer;
pub(crate) use directory::DirectoryEmbeddedViewer;
pub(crate) use image_viewer::ImageEmbeddedViewer;
pub(crate) use middlenet::MiddleNetEmbeddedViewer;
#[cfg(feature = "pdf")]
pub(crate) use pdf::PdfEmbeddedViewer;
pub(crate) use plaintext::PlaintextEmbeddedViewer;
