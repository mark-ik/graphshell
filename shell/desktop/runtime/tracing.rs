/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

/// Log an event from winit ([winit::event::Event]) at trace level.
/// - Canonical target prefix: `graphshell<winit@`
/// - Compatibility note: historical logs may contain a legacy shell prefix; runtime emission is
///   `graphshell<winit@` only.
/// - To disable tracing: RUST_LOG='graphshell<winit@=off'
/// - To enable tracing: RUST_LOG='graphshell<winit@'
/// - Recommended filters when tracing is enabled:
///   - graphshell<winit@DeviceEvent=off
///   - graphshell<winit@AboutToWait=off
///   - graphshell<winit@NewEvents(WaitCancelled)=off
///   - graphshell<winit@RedrawRequested=off
///   - graphshell<winit@UserEvent(Waker)=off
///   - graphshell<winit@WindowEvent(AxisMotion)=off
///   - graphshell<winit@WindowEvent(CursorMoved)=off
macro_rules! trace_winit_event {
    // This macro only exists to put the docs in the same file as the target prefix,
    // so the macro definition is always the same.
    ($event:expr, $($rest:tt)+) => {
        ::log::trace!(target: $crate::shell::desktop::runtime::tracing::LogTarget::log_target(&$event), $($rest)+)
    };
}

pub(crate) use trace_winit_event;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PerfSample {
    pub(crate) name: String,
    pub(crate) elapsed_us: u64,
    pub(crate) captured_at_unix_ms: u64,
}

#[cfg(feature = "tracing")]
const PERF_RING_CAPACITY: usize = 256;

#[cfg(feature = "tracing")]
static PERF_RING: std::sync::OnceLock<std::sync::Mutex<std::collections::VecDeque<PerfSample>>> =
    std::sync::OnceLock::new();

#[cfg(feature = "tracing")]
fn perf_ring() -> &'static std::sync::Mutex<std::collections::VecDeque<PerfSample>> {
    PERF_RING.get_or_init(|| std::sync::Mutex::new(std::collections::VecDeque::new()))
}

#[cfg(feature = "tracing")]
fn capture_time_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(feature = "tracing")]
fn record_perf_sample(name: String, elapsed_us: u64) {
    let mut guard = perf_ring()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.push_back(PerfSample {
        name,
        elapsed_us,
        captured_at_unix_ms: capture_time_unix_ms(),
    });
    while guard.len() > PERF_RING_CAPACITY {
        guard.pop_front();
    }
}

pub(crate) fn perf_ring_snapshot() -> Vec<PerfSample> {
    #[cfg(feature = "tracing")]
    {
        let guard = perf_ring()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        return guard.iter().cloned().collect();
    }

    #[cfg(not(feature = "tracing"))]
    {
        Vec::new()
    }
}

#[cfg(feature = "tracing")]
#[derive(Default)]
pub(crate) struct PerfRingLayer;

#[cfg(feature = "tracing")]
#[derive(Default)]
struct PerfEventVisitor {
    elapsed_us: Option<u64>,
    message: Option<String>,
}

#[cfg(feature = "tracing")]
impl tracing::field::Visit for PerfEventVisitor {
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "elapsed_us" {
            self.elapsed_us = Some(value);
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name() == "elapsed_us" && value >= 0 {
            self.elapsed_us = Some(value as u64);
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        }
    }
}

#[cfg(feature = "tracing")]
impl<S> tracing_subscriber::Layer<S> for PerfRingLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if event.metadata().target() != "graphshell::perf" {
            return;
        }

        let mut visitor = PerfEventVisitor::default();
        event.record(&mut visitor);

        let Some(elapsed_us) = visitor.elapsed_us else {
            return;
        };

        let name = visitor
            .message
            .unwrap_or_else(|| event.metadata().name().to_string());
        record_perf_sample(name, elapsed_us);
    }
}

/// Get the log target for an event, as a static string.
pub(crate) trait LogTarget {
    fn log_target(&self) -> &'static str;
}

// 2026-04-25 servo-into-verso S2b: from_winit log-target impls use
// host::event_loop::AppEvent (gated). The whole submodule is for
// the Servo+egui-host launch path's tracing instrumentation.
#[cfg(feature = "servo-engine")]
mod from_winit {
    use super::LogTarget;
    use crate::shell::desktop::host::event_loop::AppEvent;

    macro_rules! target {
        ($($name:literal)+) => {
            concat!("graphshell<winit@", $($name),+)
        };
    }

    impl LogTarget for winit::event::Event<AppEvent> {
        fn log_target(&self) -> &'static str {
            use winit::event::StartCause;
            match self {
                Self::NewEvents(start_cause) => match start_cause {
                    StartCause::ResumeTimeReached { .. } => target!("NewEvents(ResumeTimeReached)"),
                    StartCause::WaitCancelled { .. } => target!("NewEvents(WaitCancelled)"),
                    StartCause::Poll => target!("NewEvents(Poll)"),
                    StartCause::Init => target!("NewEvents(Init)"),
                },
                Self::WindowEvent { event, .. } => event.log_target(),
                Self::DeviceEvent { .. } => target!("DeviceEvent"),
                Self::UserEvent(AppEvent::Waker) => target!("UserEvent(Waker)"),
                Self::UserEvent(AppEvent::Accessibility(..)) => target!("UserEvent(Accessibility)"),
                Self::UserEvent(AppEvent::ClipExtractionCompleted { .. }) => {
                    target!("UserEvent(ClipExtractionCompleted)")
                }
                Self::UserEvent(AppEvent::ClipBatchExtractionCompleted { .. }) => {
                    target!("UserEvent(ClipBatchExtractionCompleted)")
                }
                Self::UserEvent(AppEvent::ClipInspectorPointerUpdated { .. }) => {
                    target!("UserEvent(ClipInspectorPointerUpdated)")
                }
                Self::Suspended => target!("Suspended"),
                Self::Resumed => target!("Resumed"),
                Self::AboutToWait => target!("AboutToWait"),
                Self::LoopExiting => target!("LoopExiting"),
                Self::MemoryWarning => target!("MemoryWarning"),
            }
        }
    }

    impl LogTarget for winit::event::WindowEvent {
        fn log_target(&self) -> &'static str {
            macro_rules! target_variant {
                ($name:literal) => {
                    target!("WindowEvent(" $name ")")
                };
            }
            match self {
                Self::ActivationTokenDone { .. } => target!("ActivationTokenDone"),
                Self::Resized(..) => target_variant!("Resized"),
                Self::Moved(..) => target_variant!("Moved"),
                Self::CloseRequested => target_variant!("CloseRequested"),
                Self::Destroyed => target_variant!("Destroyed"),
                Self::DroppedFile(..) => target_variant!("DroppedFile"),
                Self::HoveredFile(..) => target_variant!("HoveredFile"),
                Self::HoveredFileCancelled => target_variant!("HoveredFileCancelled"),
                Self::Focused(..) => target_variant!("Focused"),
                Self::KeyboardInput { .. } => target_variant!("KeyboardInput"),
                Self::ModifiersChanged(..) => target_variant!("ModifiersChanged"),
                Self::Ime(..) => target_variant!("Ime"),
                Self::CursorMoved { .. } => target_variant!("CursorMoved"),
                Self::CursorEntered { .. } => target_variant!("CursorEntered"),
                Self::CursorLeft { .. } => target_variant!("CursorLeft"),
                Self::MouseWheel { .. } => target_variant!("MouseWheel"),
                Self::MouseInput { .. } => target_variant!("MouseInput"),
                Self::PanGesture { .. } => target_variant!("PanGesture"),
                Self::PinchGesture { .. } => target_variant!("PinchGesture"),
                Self::DoubleTapGesture { .. } => target_variant!("DoubleTapGesture"),
                Self::RotationGesture { .. } => target_variant!("RotationGesture"),
                Self::TouchpadPressure { .. } => target_variant!("TouchpadPressure"),
                Self::AxisMotion { .. } => target_variant!("AxisMotion"),
                Self::Touch(..) => target_variant!("Touch"),
                Self::ScaleFactorChanged { .. } => target_variant!("ScaleFactorChanged"),
                Self::ThemeChanged(..) => target_variant!("ThemeChanged"),
                Self::Occluded(..) => target_variant!("Occluded"),
                Self::RedrawRequested => target!("RedrawRequested"),
            }
        }
    }
}
