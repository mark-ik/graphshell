use super::*;

impl GraphBrowserApp {
    pub fn enqueue_workbench_intent(&mut self, intent: WorkbenchIntent) {
        self.workspace.pending_workbench_intents.push(intent);
    }

    pub fn extend_workbench_intents<I>(&mut self, intents: I)
    where
        I: IntoIterator<Item = WorkbenchIntent>,
    {
        self.workspace.pending_workbench_intents.extend(intents);
    }

    pub fn take_pending_workbench_intents(&mut self) -> Vec<WorkbenchIntent> {
        std::mem::take(&mut self.workspace.pending_workbench_intents)
    }

    #[cfg(test)]
    pub fn pending_workbench_intent_count_for_tests(&self) -> usize {
        self.workspace.pending_workbench_intents.len()
    }
}
