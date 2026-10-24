use crate::ipc::contracts::*;
pub fn current(current: nooboard_core::CurrentClipboard) -> Clipboard {
    Clipboard {
        revision: current.revision.to_string(),
        kind: current.kind.into(),
        text: current.text,
        source: current.source,
        copied_at: current.copied_at_ms,
        files: current.files,
        preview: current.preview,
        image_width: current.image_width,
        image_height: current.image_height,
    }
}
pub fn activities(activities: Vec<nooboard_core::ActivityRecord>) -> Vec<ActivityRecord> {
    activities
        .into_iter()
        .map(|a| ActivityRecord {
            sequence: a.sequence.to_string(),
            kind: a.kind.into(),
            summary: a.summary,
            at: a.at_ms,
            source: a.source,
            device_name: a.device_name,
            message_id: a.message_id.map(Into::into),
            content_task: a.content_task,
            content_node: a.content_node,
            content_stage: a.content_stage.map(Into::into),
        })
        .collect()
}
