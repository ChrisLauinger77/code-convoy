//! Correlate the background-task follow-up turns observed in Claude 2.1.300.
//! This is output validation only; process ownership stays with the shared runner.
use serde_json::Value;

#[derive(Default)]
pub(super) struct Continuation {
    tasks: Vec<Task>,
    answering: bool,
}

struct Task {
    id: String,
    run: String,
    session: String,
    notified: bool,
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
        self.tasks.is_empty() && !self.answering
    }

    // False is sticky in the caller. Neither a later success nor a matching
    // notification can repair malformed, failed or uncorrelated output.
    pub(super) fn observe(&mut self, event: &Value, previous: Option<&Value>) -> bool {
        if let Some(previous) = previous {
            if event.get("session_id").is_some() && event["session_id"] != previous["session_id"] {
                return false;
            }
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
                            && self
                                .tasks
                                .iter()
                                .any(|task| task.matches(event) && !task.notified)
                    }
                    Some("task_notification") => self.notify(event),
                    Some("background_tasks_changed") if !self.tasks.is_empty() => {
                        event["tasks"].as_array().is_some_and(|tasks| {
                            // A snapshot cannot discharge an outstanding task or
                            // authorize a new one; only its result does that.
                            tasks.iter().all(|entry| {
                                self.tasks.iter().any(|task| {
                                    !task.notified
                                        && entry["task_id"] == task.id
                                        && entry["run_id"] == task.run
                                })
                            })
                        })
                    }
                    Some("init") if self.tasks.iter().any(|task| task.notified) => {
                        self.answering = true;
                        true
                    }
                    _ => false,
                };
            }
        }

        if event["type"] == "result" {
            return (event.get("result_index").is_none()
                || event["result_index"].as_u64() == Some(0))
                && (event["origin"].is_null() || event["origin"]["kind"] == "human")
                && self
                    .tasks
                    .iter()
                    .all(|task| event["session_id"] == task.session);
        }
        if event["type"] == "system" {
            if event["subtype"] == "task_started" && event["is_backgrounded"] == true {
                let (Some(id), Some(run), Some(session)) = (
                    identifier(&event["task_id"]),
                    identifier(&event["run_id"]),
                    identifier(&event["session_id"]),
                ) else {
                    return false;
                };
                // Bound retained correlation data as well as individual JSON records.
                if self.tasks.len() >= 128
                    || self
                        .tasks
                        .iter()
                        .any(|task| task.id == id || task.run == run)
                {
                    return false;
                }
                self.tasks.push(Task {
                    id: id.into(),
                    run: run.into(),
                    session: session.into(),
                    notified: false,
                });
            } else if event["subtype"] == "task_notification"
                && self
                    .tasks
                    .iter()
                    .any(|task| event["task_id"] == task.id || event["run_id"] == task.run)
            {
                return self.notify(event);
            }
        }
        true
    }

    fn notify(&mut self, event: &Value) -> bool {
        let Some(index) = self
            .tasks
            .iter()
            .position(|task| task.matches(event) && !task.notified)
        else {
            return false;
        };
        if event["status"] != "completed" {
            return false;
        }
        let mut task = self.tasks.remove(index);
        task.notified = true;
        // Notification order, not launch order, determines result order.
        // Even a notification before the initial result needs an attributed
        // answer; the initial human-prompt result cannot prove it was handled.
        self.tasks.push(task);
        true
    }

    fn finish_turn(&mut self, event: &Value, previous: &Value) -> bool {
        let Some(index) = self.tasks.iter().position(|task| task.notified) else {
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
        self.tasks.remove(index);
        // The SDK can coalesce notifications into one answer, with a result
        // for each notification. All of them are still required and validated.
        self.answering = self.tasks.iter().any(|task| task.notified);
        // A coalesced zero-turn placeholder is not the answer to the batch.
        self.answering || event["num_turns"] != 0 || event["result"] != ""
    }
}

fn identifier(value: &Value) -> Option<&str> {
    value
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 256)
}
