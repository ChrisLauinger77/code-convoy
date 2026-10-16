//! Correlate the background-task follow-up turns observed in Claude 2.1.300.
//! This is output validation only; process ownership stays with the shared runner.
use serde_json::Value;

#[derive(Default)]
pub(super) struct Continuation {
    tasks: Vec<Task>,
    answering: bool,
    queued_turns_pending: bool,
}

struct Task {
    id: String,
    run: String,
    session: String,
    completion: TaskCompletion,
    start_seen: bool,
    listed: bool,
}

#[derive(PartialEq, Eq)]
enum TaskCompletion {
    Pending,
    Notified,
    Answered,
}

impl Task {
    fn matches(&self, event: &Value) -> bool {
        event["task_id"] == self.id
            && event["run_id"] == self.run
            && event["session_id"] == self.session
    }
}

impl Continuation {
    pub(super) fn complete(&self) -> bool {
        self.tasks
            .iter()
            .all(|task| task.completion == TaskCompletion::Answered)
            && !self.answering
            && !self.queued_turns_pending
    }

    // False is sticky in the caller. Neither a later success nor a matching
    // notification can repair malformed, failed or uncorrelated output.
    pub(super) fn observe(&mut self, event: &Value, previous: Option<&Value>) -> bool {
        if event["type"] == "result"
            && let Some(queued) = event["queued_turn_count"].as_u64()
        {
            // A positive count promises another result, not a failed turn.
            // Only an explicit zero can discharge it; omission cannot. Counts
            // never authorize turns without the existing task correlation.
            self.queued_turns_pending = queued > 0;
        }
        if previous.is_some_and(|previous| {
            event.get("session_id").is_some() && event["session_id"] != previous["session_id"]
        }) {
            return false;
        }
        if let Some(valid) = self.background_event(event) {
            return valid;
        }
        if let Some(previous) = previous {
            if event["type"] == "result" {
                return self.finish_turn(event, previous);
            }
            if !self.answering {
                if super::ui_render_notification(event) {
                    return true;
                }
                if event["type"] != "system"
                    || event["session_id"] != previous["session_id"]
                    || !event["error"].is_null()
                    || !event["errors"].is_null()
                    || (event.get("is_error").is_some() && event["is_error"] != false)
                {
                    return false;
                }
                return match event["subtype"].as_str() {
                    Some("task_updated") => {
                        event["patch"]["status"] == "completed"
                            && self.tasks.iter().any(|task| {
                                task.matches(event) && task.completion == TaskCompletion::Pending
                            })
                    }
                    Some("task_notification") => self.notify(event),
                    Some("init")
                        if self
                            .tasks
                            .iter()
                            .any(|task| task.completion == TaskCompletion::Notified) =>
                    {
                        self.answering = true;
                        true
                    }
                    _ => false,
                };
            }
        }

        // Follow-up results return through finish_turn above; this check only
        // validates the initial result, whose index remains optional.
        if event["type"] == "result" {
            return (event.get("result_index").is_none()
                || event["result_index"].as_u64() == Some(0))
                && (event["origin"].is_null() || event["origin"]["kind"] == "human")
                && self
                    .tasks
                    .iter()
                    .all(|task| event["session_id"] == task.session);
        }
        if event["type"] == "system"
            && event["subtype"] == "task_notification"
            && self
                .tasks
                .iter()
                .any(|task| event["task_id"] == task.id || event["run_id"] == task.run)
        {
            return self.notify(event);
        }
        true
    }

    fn background_event(&mut self, event: &Value) -> Option<bool> {
        if event["type"] != "system" {
            return None;
        }
        let started = event["subtype"] == "task_started" && event["is_backgrounded"] == true;
        let promoted =
            event["subtype"] == "task_updated" && event["patch"]["is_backgrounded"] == true;
        let snapshot = event["subtype"] == "background_tasks_changed";
        if !started && !promoted && !snapshot {
            return None;
        }
        if identifier(&event["session_id"]).is_none()
            || !event["error"].is_null()
            || !event["errors"].is_null()
            || (event.get("is_error").is_some() && event["is_error"] != false)
            || (promoted
                && (!event["patch"]["error"].is_null()
                    || (event["patch"].get("status").is_some()
                        && !matches!(
                            event["patch"]["status"].as_str(),
                            Some("pending" | "running" | "completed" | "paused")
                        ))))
        {
            return Some(false);
        }
        if !snapshot {
            return Some(self.register(event, &event["session_id"], started, false));
        }
        let Some(tasks) = event["tasks"].as_array().filter(|tasks| tasks.len() <= 128) else {
            return Some(false);
        };
        // SDK snapshots may precede their start/update bookends. Merge positive
        // membership evidence, but never clear an obligation when a run leaves.
        for task in &mut self.tasks {
            task.listed = tasks.iter().any(|entry| entry["run_id"] == task.run);
        }
        self.tasks
            .retain(|task| task.listed || task.completion != TaskCompletion::Answered);
        Some(
            tasks
                .iter()
                .all(|task| self.register(task, &event["session_id"], false, true)),
        )
    }

    fn register(&mut self, event: &Value, session: &Value, started: bool, listed: bool) -> bool {
        let (Some(id), Some(run), Some(session)) = (
            identifier(&event["task_id"]),
            identifier(&event["run_id"]),
            identifier(session),
        ) else {
            return false;
        };
        if let Some(task) = self.tasks.iter_mut().find(|task| task.run == run) {
            if task.id != id
                || task.session != session
                || (started && (task.start_seen || task.completion == TaskCompletion::Answered))
            {
                return false;
            }
            task.start_seen |= started;
            task.listed |= listed;
            return true;
        }
        // Task IDs survive resume; bound independently retained run identities.
        if self.tasks.len() >= 128 {
            return false;
        }
        self.tasks.push(Task {
            id: id.into(),
            run: run.into(),
            session: session.into(),
            completion: TaskCompletion::Pending,
            start_seen: started,
            listed,
        });
        true
    }

    fn notify(&mut self, event: &Value) -> bool {
        let Some(index) = self
            .tasks
            .iter()
            .position(|task| task.matches(event) && task.completion == TaskCompletion::Pending)
        else {
            return false;
        };
        if event["status"] != "completed" {
            return false;
        }
        let mut task = self.tasks.remove(index);
        task.completion = TaskCompletion::Notified;
        // Notification order, not launch order, determines result order.
        // Even a notification before the initial result needs an attributed
        // answer; the initial human-prompt result cannot prove it was handled.
        self.tasks.push(task);
        true
    }

    fn finish_turn(&mut self, event: &Value, previous: &Value) -> bool {
        let Some(index) = self
            .tasks
            .iter()
            .position(|task| task.completion == TaskCompletion::Notified)
        else {
            return false;
        };
        let task = &self.tasks[index];
        if !self.answering
            || event["session_id"] != task.session
            || event["origin"]["kind"] != "task-notification"
            || event["origin"]["producer"] != "session-task"
            || event["origin"]["runId"] != task.run
            || previous["result_index"]
                .as_u64()
                .and_then(|n| n.checked_add(1))
                .is_none_or(|next| event["result_index"].as_u64() != Some(next))
        {
            return false;
        }
        if self.tasks[index].listed {
            // Membership can lag the result. Retain the answered identity until
            // it leaves the level, so a late snapshot cannot reopen the answer.
            self.tasks[index].completion = TaskCompletion::Answered;
        } else {
            self.tasks.remove(index);
        }
        // The SDK can coalesce notifications into one answer, with a result
        // for each notification. All of them are still required and validated.
        self.answering = self
            .tasks
            .iter()
            .any(|task| task.completion == TaskCompletion::Notified);
        // A coalesced zero-turn placeholder is not the answer to the batch.
        self.answering || event["num_turns"] != 0 || event["result"] != ""
    }
}

fn identifier(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 256)
}
