#[cfg(test)]
mod tests {
    use super::super::{Gui, UpdateFrameStage};

    #[test]
    fn gui_test_module_compiles() {
        assert!(true);
    }

    #[test]
    fn update_frame_stage_sequence_is_canonical() {
        let sequence = Gui::update_frame_stage_sequence();
        assert!(Gui::is_canonical_update_frame_stage_sequence(sequence));
    }

    #[test]
    fn update_frame_stage_sequence_has_expected_order() {
        let sequence = Gui::update_frame_stage_sequence();
        assert_eq!(sequence.len(), 6);
        assert_eq!(sequence[0], UpdateFrameStage::Prelude);
        assert_eq!(sequence[1], UpdateFrameStage::PreFrameInit);
        assert_eq!(sequence[2], UpdateFrameStage::GraphSearchAndKeyboard);
        assert_eq!(sequence[3], UpdateFrameStage::ToolbarAndGraphSearchWindow);
        assert_eq!(sequence[4], UpdateFrameStage::SemanticAndPostRender);
        assert_eq!(sequence[5], UpdateFrameStage::Finalize);
    }
}
