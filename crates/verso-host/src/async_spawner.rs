/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crossbeam_channel::bounded;
use futures_util::future::BoxFuture;
use graphshell_core::async_host::{
    AsyncSpawner, BoxedBlockingWork, ErasedBlockingResult, SpawnError,
};
use tokio::runtime::Handle;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct TokioAsyncSpawner {
    runtime_handle: Option<Handle>,
    cancel: CancellationToken,
    handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
    worker_count: Arc<AtomicUsize>,
}

impl TokioAsyncSpawner {
    pub fn new(runtime_handle: Option<Handle>) -> Self {
        Self {
            runtime_handle,
            cancel: CancellationToken::new(),
            handles: Arc::new(Mutex::new(Vec::new())),
            worker_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn runtime_handle(&self) -> Handle {
        self.runtime_handle.clone().unwrap_or_else(|| {
            panic!(
                "ControlPanel worker spawn requires a Tokio runtime handle; construct with ControlPanel::new_with_runtime(...) or create ControlPanel inside an active Tokio runtime"
            )
        })
    }

    #[cfg(test)]
    pub fn worker_count(&self) -> usize {
        self.worker_count.load(Ordering::SeqCst)
    }
}

impl AsyncSpawner for TokioAsyncSpawner {
    fn spawn_supervised(
        &self,
        _label: &'static str,
        task: BoxFuture<'static, ()>,
    ) -> Result<(), SpawnError> {
        if self.cancel.is_cancelled() {
            return Err(SpawnError::ShuttingDown);
        }

        let handle = self.runtime_handle();
        let join_handle = handle.spawn(task);
        self.handles
            .lock()
            .expect("tokio async spawner handle lock poisoned")
            .push(join_handle);
        self.worker_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn spawn_blocking_erased(
        &self,
        label: &'static str,
        work: BoxedBlockingWork,
    ) -> Result<crossbeam_channel::Receiver<ErasedBlockingResult>, SpawnError> {
        let cancel = self.cancel.clone();
        let (tx, rx) = bounded(1);
        self.spawn_supervised(
            label,
            Box::pin(async move {
                if cancel.is_cancelled() {
                    return;
                }

                let result = tokio::task::spawn_blocking(work).await;
                if cancel.is_cancelled() {
                    return;
                }

                if let Ok(value) = result {
                    let _ = tx.send(value);
                }
            }),
        )?;
        Ok(rx)
    }

    fn request_cancel(&self) {
        self.cancel.cancel();
    }

    fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    fn shutdown(&self) -> BoxFuture<'static, ()> {
        let cancel = self.cancel.clone();
        let handles = Arc::clone(&self.handles);
        let worker_count = Arc::clone(&self.worker_count);
        Box::pin(async move {
            cancel.cancel();
            let pending = {
                let mut guard = handles
                    .lock()
                    .expect("tokio async spawner handle lock poisoned");
                std::mem::take(&mut *guard)
            };
            for handle in pending {
                let _ = handle.await;
            }
            worker_count.store(0, Ordering::SeqCst);
        })
    }
}