use super::*;

fn promoted_events() -> Vec<Value> {
    let mut events = background_events();
    events[4]["is_backgrounded"] = json!(false);
    events.insert(
        5,
        json!({
            "type":"system", "subtype":"task_updated", "session_id":"fixture",
            "task_id":"background-task", "run_id":"background-run",
            "patch":{"is_backgrounded":true, "status":"running"}
        }),
    );
    events.retain(|event| event["subtype"] != "background_tasks_changed");
    events
}

#[test]
fn foreground_tasks_promoted_to_background_require_a_correlated_follow_up() {
    let events = promoted_events();
    assert_eq!(background_outcome(&events), JobStatus::Succeeded);
    for end in 0..events.len() {
        assert_eq!(
            background_outcome(&events[..end]),
            JobStatus::Failed,
            "prefix {end}"
        );
    }
    let mut late = events.clone();
    let promotion = late
        .iter()
        .position(|event| event["patch"]["is_backgrounded"] == true)
        .unwrap();
    let promotion = late.remove(promotion);
    let result = late
        .iter()
        .position(|event| event["result_index"] == 0)
        .unwrap();
    late.insert(result + 1, promotion.clone());
    assert_eq!(background_outcome(&late), JobStatus::Succeeded);
    late.insert(result + 2, promotion);
    assert_eq!(background_outcome(&late), JobStatus::Succeeded);

    // Foreground-only work creates no background obligation or retained entry.
    let original = background_events();
    let mut foreground = Vec::new();
    for n in 0..256 {
        for index in [16, 17] {
            let mut event = original[index].clone();
            event["task_id"] = json!(format!("foreground-{n}"));
            event["run_id"] = json!(format!("foreground-run-{n}"));
            foreground.push(event);
        }
    }
    foreground.push(success());
    assert_eq!(background_outcome(&foreground), JobStatus::Succeeded);
}

#[test]
fn background_levels_merge_bookends_but_do_not_complete_or_reopen_answers() {
    let original = background_events();
    let mut no_start = original.clone();
    no_start.remove(4);
    assert_eq!(background_outcome(&no_start), JobStatus::Succeeded);
    let mut foreground_start = original.clone();
    foreground_start[4]["is_backgrounded"] = json!(false);
    assert_eq!(background_outcome(&foreground_start), JobStatus::Succeeded);
    let membership = foreground_start.remove(3);
    foreground_start.insert(4, membership);
    assert_eq!(background_outcome(&foreground_start), JobStatus::Succeeded);

    let mut repeated = original.clone();
    repeated.insert(5, original[3].clone());
    assert_eq!(background_outcome(&repeated), JobStatus::Succeeded);
    let mut trailing_empty = original.clone();
    trailing_empty.push(original[11].clone());
    assert_eq!(background_outcome(&trailing_empty), JobStatus::Succeeded);

    // The membership level can lag the attributed answer. A still-listed run
    // already answered must remain answered across repeated level payloads.
    let mut lagging = original.clone();
    let empty = lagging.remove(11);
    lagging.push(original[3].clone());
    lagging.push(empty);
    assert_eq!(background_outcome(&lagging), JobStatus::Succeeded);
    lagging.pop();
    assert_eq!(background_outcome(&lagging), JobStatus::Succeeded);

    let mut unfinished = original[..9].to_vec();
    unfinished.push(original[11].clone());
    assert_eq!(background_outcome(&unfinished), JobStatus::Failed);
    let mut no_evidence = original.clone();
    no_evidence[4]["is_backgrounded"] = json!(false);
    no_evidence.retain(|event| event["subtype"] != "background_tasks_changed");
    assert_eq!(background_outcome(&no_evidence), JobStatus::Failed);
}

#[test]
fn answered_membership_records_are_released_between_many_resumed_runs() {
    let original = background_events();
    let mut events = original[..11].to_vec();
    events.push(original[12].clone());
    let mut current_run = "background-run".to_owned();
    for n in 0..130 {
        let next_run = format!("resumed-{n:04}");
        let mut start = original[4].clone();
        start["run_id"] = json!(next_run);
        events.push(start);
        let mut result = original[20].clone();
        result["origin"]["runId"] = json!(current_run);
        result["result_index"] = json!(n + 1);
        events.push(result);
        let mut membership = original[3].clone();
        membership["tasks"][0]["run_id"] = json!(next_run);
        events.push(membership);
        let mut notification = original[10].clone();
        notification["run_id"] = json!(next_run);
        events.push(notification);
        events.push(original[12].clone());
        current_run = next_run;
    }
    let mut final_result = original[20].clone();
    final_result["origin"]["runId"] = json!(current_run);
    final_result["result_index"] = json!(131);
    events.push(final_result);
    events.push(original[11].clone());
    assert_eq!(background_outcome(&events), JobStatus::Succeeded);
}

#[test]
fn malformed_background_evidence_cannot_be_repaired_by_later_success() {
    for (key, value) in [
        ("task_id", json!(null)),
        ("run_id", json!("")),
        ("session_id", json!("unrelated")),
        ("is_error", json!(true)),
        ("errors", json!(["failed"])),
    ] {
        let mut events = promoted_events();
        let promotion = events
            .iter_mut()
            .find(|event| event["patch"]["is_backgrounded"] == true)
            .unwrap();
        promotion[key] = value;
        assert_eq!(
            background_outcome(&events),
            JobStatus::Failed,
            "promotion {key}"
        );
    }
    for (key, value) in [
        ("is_backgrounded", json!(false)),
        ("is_backgrounded", json!("true")),
        ("error", json!("failed")),
        ("status", json!("failed")),
        ("status", json!("killed")),
        ("status", json!(true)),
    ] {
        let mut events = promoted_events();
        let promotion = events
            .iter_mut()
            .find(|event| event["patch"]["is_backgrounded"] == true)
            .unwrap();
        promotion["patch"][key] = value;
        assert_eq!(
            background_outcome(&events),
            JobStatus::Failed,
            "patch {key}"
        );
    }
    let original = background_events();
    for entries in [
        json!(null),
        json!([null]),
        json!([{"task_id":"background-task"}]),
        json!([{"task_id":null,"run_id":"background-run"}]),
        json!([{"task_id":"background-task","run_id":""}]),
        json!([
            {"task_id":"background-task","run_id":"background-run"},
            {"task_id":"unrelated","run_id":"background-run"}
        ]),
    ] {
        let mut events = original.clone();
        events[3]["tasks"] = entries;
        assert_eq!(background_outcome(&events), JobStatus::Failed);
    }
}
