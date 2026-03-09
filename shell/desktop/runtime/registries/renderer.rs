use std::collections::HashMap;
use std::time::Instant;

use crate::app::RendererId;
use crate::graph::NodeKey;
use crate::shell::desktop::workbench::pane_model::PaneId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaneAttachment {
    pub(crate) pane_id: PaneId,
    pub(crate) renderer_id: RendererId,
    pub(crate) attached_at: Instant,
    pub(crate) node_key: Option<NodeKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RendererRegistryError {
    PaneAlreadyAttached {
        pane_id: PaneId,
        existing_renderer_id: RendererId,
    },
    RendererAlreadyAttached {
        renderer_id: RendererId,
        existing_pane_id: PaneId,
    },
}

#[derive(Default)]
pub(crate) struct RendererRegistry {
    by_pane: HashMap<PaneId, PaneAttachment>,
    by_renderer: HashMap<RendererId, PaneId>,
}

impl RendererRegistry {
    pub(crate) fn accept(
        &mut self,
        pane_id: PaneId,
        renderer_id: RendererId,
        node_key: Option<NodeKey>,
    ) -> Result<(), RendererRegistryError> {
        if let Some(existing) = self.by_pane.get(&pane_id) {
            return Err(RendererRegistryError::PaneAlreadyAttached {
                pane_id,
                existing_renderer_id: existing.renderer_id,
            });
        }

        if let Some(existing_pane_id) = self.by_renderer.get(&renderer_id).copied() {
            return Err(RendererRegistryError::RendererAlreadyAttached {
                renderer_id,
                existing_pane_id,
            });
        }

        let attachment = PaneAttachment {
            pane_id,
            renderer_id,
            attached_at: Instant::now(),
            node_key,
        };
        self.by_renderer.insert(renderer_id, pane_id);
        self.by_pane.insert(pane_id, attachment);
        Ok(())
    }

    pub(crate) fn detach(&mut self, renderer_id: RendererId) -> Option<PaneAttachment> {
        let pane_id = self.by_renderer.remove(&renderer_id)?;
        self.by_pane.remove(&pane_id)
    }

    pub(crate) fn renderer_for_pane(&self, pane_id: &PaneId) -> Option<&PaneAttachment> {
        self.by_pane.get(pane_id)
    }

    pub(crate) fn pane_for_renderer(&self, renderer_id: &RendererId) -> Option<PaneId> {
        self.by_renderer.get(renderer_id).copied()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.by_pane.is_empty() && self.by_renderer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn renderer_id() -> RendererId {
        #[cfg(not(target_os = "ios"))]
        {
            thread_local! {
                static NS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
            }
            NS_INSTALLED.with(|cell| {
                if !cell.get() {
                    base::id::PipelineNamespace::install(base::id::PipelineNamespaceId(42));
                    cell.set(true);
                }
            });
            servo::WebViewId::new(base::id::PainterId::next())
        }

        #[cfg(target_os = "ios")]
        {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            RendererId(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        }
    }

    #[test]
    fn accept_registers_bidirectional_attachment() {
        let mut registry = RendererRegistry::default();
        let pane_id = PaneId::new();
        let renderer_id = renderer_id();

        registry.accept(pane_id, renderer_id, None).unwrap();

        let attachment = registry.renderer_for_pane(&pane_id).unwrap();
        assert_eq!(attachment.pane_id, pane_id);
        assert_eq!(attachment.renderer_id, renderer_id);
        assert_eq!(registry.pane_for_renderer(&renderer_id), Some(pane_id));
    }

    #[test]
    fn detach_removes_attachment_from_both_indexes() {
        let mut registry = RendererRegistry::default();
        let pane_id = PaneId::new();
        let renderer_id = renderer_id();

        registry.accept(pane_id, renderer_id, None).unwrap();

        let removed = registry.detach(renderer_id).unwrap();
        assert_eq!(removed.pane_id, pane_id);
        assert_eq!(removed.renderer_id, renderer_id);
        assert!(registry.renderer_for_pane(&pane_id).is_none());
        assert!(registry.pane_for_renderer(&renderer_id).is_none());
        assert!(registry.is_empty());
    }

    #[test]
    fn accept_rejects_duplicate_pane_attachment() {
        let mut registry = RendererRegistry::default();
        let pane_id = PaneId::new();
        let existing_renderer_id = renderer_id();

        registry.accept(pane_id, existing_renderer_id, None).unwrap();

        let error = registry.accept(pane_id, renderer_id(), None).unwrap_err();
        assert_eq!(
            error,
            RendererRegistryError::PaneAlreadyAttached {
                pane_id,
                existing_renderer_id,
            }
        );
    }

    #[test]
    fn accept_rejects_duplicate_renderer_attachment() {
        let mut registry = RendererRegistry::default();
        let pane_a = PaneId::new();
        let pane_b = PaneId::new();
        let renderer_id = renderer_id();

        registry.accept(pane_a, renderer_id, None).unwrap();

        let error = registry.accept(pane_b, renderer_id, None).unwrap_err();
        assert_eq!(
            error,
            RendererRegistryError::RendererAlreadyAttached {
                renderer_id,
                existing_pane_id: pane_a,
            }
        );
    }
}