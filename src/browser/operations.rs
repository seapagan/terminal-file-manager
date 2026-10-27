use crate::{
    input::Controller,
    operations::{Dialog, Intent, Queue},
    workers::Pool,
};
use std::time::Duration;

#[cfg(test)]
mod tests;

pub(super) fn collect(controller: &mut Controller, queue: &mut Queue, pool: &mut Pool) -> bool {
    let mut changed = false;
    for _ in 0..12 {
        let Some(update) = queue.next(Duration::ZERO) else {
            break;
        };
        for request in controller.complete_operation(update) {
            pool.submit(request);
        }
        changed = true;
    }
    changed
}

pub(super) fn submit(controller: &mut Controller, queue: &mut Queue) {
    for intent in std::mem::take(&mut controller.app.operations.outbox) {
        let result = match intent {
            Intent::Submit(plan, options) => queue
                .submit(plan.clone(), options)
                .map(|id| controller.register_operation(id, plan, options, None)),
            Intent::Preview(plan) => queue.preview(plan).map(|id| {
                if let Some(Dialog::Operation(dialog)) = &mut controller.app.operations.dialog {
                    dialog.preview_id = Some(id);
                }
            }),
            Intent::Cancel(id) => {
                queue.cancel(id);
                Ok(())
            }
            Intent::Retry(id) => retry(controller, queue, id),
            Intent::Reconcile(id) => reconcile(controller, queue, id),
            Intent::SubmitRetry(plan, options, parent) => queue
                .submit(plan.clone(), options)
                .map(|id| controller.register_operation(id, plan, options, Some(parent))),
        };
        if let Err(error) = result {
            controller
                .app
                .operations
                .fail(error.to_string(), controller.app.now);
            controller.app.status = format!("Not started: {error} — F9 details");
        }
    }
}

fn reconcile(controller: &mut Controller, queue: &mut Queue, id: u64) -> std::io::Result<()> {
    let job = controller
        .app
        .operations
        .history
        .iter_mut()
        .find(|job| job.id == id)
        .ok_or_else(|| std::io::Error::other("Unknown job"))?;
    if !job.can_reconcile() {
        return Err(std::io::Error::other(
            "No reconcilable unknown outcome with retained evidence",
        ));
    }
    let record = job
        .record
        .as_ref()
        .filter(|record| record.unresolved())
        .ok_or_else(|| {
            std::io::Error::other("Only an acknowledged unknown job needs reconciliation")
        })?;
    queue.reconcile(id, job.plan.clone(), record.clone())?;
    job.state = crate::operations::JobState::Queued;
    job.progress = None;
    controller.app.status = format!("Job {id}: checking retained identities and owned staging");
    Ok(())
}

fn retry(controller: &mut Controller, queue: &mut Queue, id: u64) -> std::io::Result<()> {
    let job = controller
        .app
        .operations
        .history
        .iter()
        .find(|job| job.id == id && job.can_retry())
        .ok_or_else(|| std::io::Error::other("No authorised incomplete attempt to retry"))?;
    let record = job.record.as_ref().ok_or_else(|| {
        std::io::Error::other("Job still active; wait for acknowledgement before retry")
    })?;
    let plan = job.plan.retry(record)?;
    let options = job.options;
    if matches!(
        plan.operation,
        crate::operations::Operation::Delete | crate::operations::Operation::Trash
    ) {
        let preview_id = queue.preview(plan.clone())?;
        let base = plan.operands[0]
            .path
            .parent()
            .unwrap_or(&plan.operands[0].path)
            .to_owned();
        controller.app.operations.show_history = false;
        controller.app.operations.scroll = 0;
        controller.app.operations.dialog = Some(Dialog::Operation(Box::new(
            crate::operations::OperationDialog {
                plan,
                pane: controller.app.active,
                native_target: base.clone(),
                base,
                text: String::new(),
                incoming: String::new(),
                edited: false,
                field: 0,
                options,
                ready: false,
                preview_id: Some(preview_id),
                track_scope: false,
                retry_of: Some(id),
                error: None,
            },
        )));
        return Ok(());
    }
    let next = queue.submit(plan.clone(), options)?;
    controller.register_operation(next, plan, options, Some(id));
    Ok(())
}
