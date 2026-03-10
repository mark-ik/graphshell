use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::app::agents::tag_suggester::{self, AGENT_ID_TAG_SUGGESTER};
use crate::shell::desktop::runtime::control_panel::QueuedIntent;

use super::RegistryRuntime;
use super::signal_routing::{AsyncSignalSubscription, SignalTopic};

pub(crate) const AGENT_ID_GRAPH_SUMMARISER: &str = "agent:graph_summariser";

pub(crate) type AgentTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentCapability {
    ReadNavigationSignals,
    SuggestNodeTags,
    SummarizeGraph,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentSchedule {
    OnDemand,
    Triggered(SignalTopic),
}

pub(crate) struct AgentContext {
    pub(crate) intent_tx: mpsc::Sender<QueuedIntent>,
    pub(crate) signal_rx: AsyncSignalSubscription,
    pub(crate) cancel: CancellationToken,
    pub(crate) registries: Arc<RegistryRuntime>,
}

pub(crate) struct AgentHandle {
    pub(crate) task: AgentTask,
}

impl AgentHandle {
    pub(crate) fn from_future(
        task: impl Future<Output = ()> + Send + 'static,
    ) -> Self {
        Self {
            task: Box::pin(task),
        }
    }
}

pub(crate) trait Agent: Send {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn declared_capabilities(&self) -> Vec<AgentCapability>;
    fn spawn(self: Box<Self>, context: AgentContext) -> AgentHandle;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentDescriptor {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) capabilities: Vec<AgentCapability>,
    pub(crate) schedule: AgentSchedule,
}

struct RegisteredAgent {
    descriptor: AgentDescriptor,
    factory: fn() -> Box<dyn Agent>,
}

pub(crate) struct AgentRegistry {
    registered: HashMap<String, RegisteredAgent>,
}

impl AgentRegistry {
    pub(crate) fn register(
        &mut self,
        descriptor: AgentDescriptor,
        factory: fn() -> Box<dyn Agent>,
    ) -> Result<(), String> {
        let agent_id = descriptor.id.trim().to_ascii_lowercase();
        if self.registered.contains_key(&agent_id) {
            return Err(format!("agent already registered: {agent_id}"));
        }
        self.registered.insert(
            agent_id,
            RegisteredAgent {
                descriptor,
                factory,
            },
        );
        Ok(())
    }

    pub(crate) fn describe(&self, agent_id: &str) -> Option<AgentDescriptor> {
        self.registered
            .get(&agent_id.trim().to_ascii_lowercase())
            .map(|registered| registered.descriptor.clone())
    }

    pub(crate) fn descriptors(&self) -> Vec<AgentDescriptor> {
        let mut descriptors = self
            .registered
            .values()
            .map(|registered| registered.descriptor.clone())
            .collect::<Vec<_>>();
        descriptors.sort_by(|a, b| a.id.cmp(&b.id));
        descriptors
    }

    pub(crate) fn instantiate(&self, agent_id: &str) -> Option<Box<dyn Agent>> {
        self.registered
            .get(&agent_id.trim().to_ascii_lowercase())
            .map(|registered| (registered.factory)())
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        let mut registry = Self {
            registered: HashMap::new(),
        };
        registry
            .register(
                AgentDescriptor {
                    id: AGENT_ID_GRAPH_SUMMARISER.to_string(),
                    display_name: "Graph summariser".to_string(),
                    capabilities: vec![AgentCapability::SummarizeGraph],
                    schedule: AgentSchedule::OnDemand,
                },
                || Box::new(GraphSummariserAgent),
            )
            .expect("graph summariser registration should succeed");
        registry
            .register(
                AgentDescriptor {
                    id: AGENT_ID_TAG_SUGGESTER.to_string(),
                    display_name: "Tag suggester".to_string(),
                    capabilities: vec![
                        AgentCapability::ReadNavigationSignals,
                        AgentCapability::SuggestNodeTags,
                    ],
                    schedule: AgentSchedule::Triggered(SignalTopic::Navigation),
                },
                tag_suggester::instantiate,
            )
            .expect("tag suggester registration should succeed");
        registry
    }
}

struct GraphSummariserAgent;

impl Agent for GraphSummariserAgent {
    fn id(&self) -> &'static str {
        AGENT_ID_GRAPH_SUMMARISER
    }

    fn display_name(&self) -> &'static str {
        "Graph summariser"
    }

    fn declared_capabilities(&self) -> Vec<AgentCapability> {
        vec![AgentCapability::SummarizeGraph]
    }

    fn spawn(self: Box<Self>, context: AgentContext) -> AgentHandle {
        AgentHandle::from_future(async move {
            context.cancel.cancelled().await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_registry_registers_builtin_descriptors() {
        let registry = AgentRegistry::default();
        let descriptors = registry.descriptors();

        assert_eq!(descriptors.len(), 2);
        assert!(descriptors.iter().any(|descriptor| descriptor.id == AGENT_ID_GRAPH_SUMMARISER));
        assert!(descriptors.iter().any(|descriptor| descriptor.id == AGENT_ID_TAG_SUGGESTER));
    }
}
