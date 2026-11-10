//! Project/library 接続の [`MediaReferenceProbe`] 実装（#116 RCD-02）。
//!
//! staging media の Discard 可否を project 側の entity 参照から
//! fail-closed に判定する adapter。`synvelis-recording` は project
//! snapshot（live・undo/redo history・saved HEAD）を read-only で
//! 調べ、project/store への書き込みは行わない。
//!
//! 参照判定は recording registry の stream 結合 entity
//! （`applied_commits`・`takes.stream_id`・`segments.stream_id`）を
//! 正本とする。clip・comp region・take group はこれら entity 経由で
//! のみ media source へ到達し得る（commit が take/segment を必ず
//! 生成し、手動登録 op も同じ結合を持つ）ため推移的に覆われる。
//! 判定不能（HEAD の schema 不一致・decode 失敗・store error・
//! `project_id` 不一致・tagged ID 破損）は全て `Unknown` で Discard
//! を拒否する。

use synvelis_project::{
    CaptureStreamId, LaneId, Project, ProjectController, ProjectStore, RecordingSessionId,
    SCHEMA_VERSION, SessionState, SnapshotEnvelope,
};

use crate::recovery::{DiscardCandidate, MediaReferenceProbe, ReferenceCheck};

/// [`ProjectController`] の live/履歴と [`ProjectStore`] の saved
/// HEAD を調べる [`MediaReferenceProbe`]。
///
/// `Clear` は「live project・undo/redo 履歴・saved HEAD のいずれも
/// 当該 session/stream を参照していない」場合に限る。saved HEAD の
/// schema が現行と異なる場合は migration 未対応として `Unknown`
/// を返す（旧 snapshot を推測で読まない）。
pub struct ProjectMediaProbe<'a> {
    controller: &'a ProjectController,
    store: &'a dyn ProjectStore,
}

impl<'a> ProjectMediaProbe<'a> {
    /// probe を構成する。`store` は saved HEAD snapshot の参照
    /// 判定に使う（`InMemoryStore`/`DirectoryBundleStore` いずれも可）。
    #[must_use]
    pub fn new(controller: &'a ProjectController, store: &'a dyn ProjectStore) -> Self {
        Self { controller, store }
    }

    /// saved HEAD snapshot を decode する。`Ok(None)` は HEAD 無し。
    /// store error・decode 失敗・schema 不一致は `Err`（= `Unknown`）。
    fn saved_head_project(&self) -> Result<Option<Project>, String> {
        let head = self
            .store
            .head()
            .map_err(|e| format!("store head read failed: {e}"))?;
        let Some(name) = head else {
            return Ok(None);
        };
        let bytes = self
            .store
            .read_snapshot(&name)
            .map_err(|e| format!("saved snapshot {name} unreadable: {e}"))?;
        let envelope: SnapshotEnvelope = serde_json::from_slice(&bytes)
            .map_err(|e| format!("saved snapshot {name} undecodable: {e}"))?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "saved snapshot schema {} requires migration (current {SCHEMA_VERSION})",
                envelope.schema_version
            ));
        }
        if envelope.project_id != envelope.project.project_id {
            return Err(format!(
                "saved snapshot {name} project_id mismatch (envelope vs body)"
            ));
        }
        Ok(Some(envelope.project))
    }
}

/// 1 project 状態における stream 結合参照を調べる。
/// `applied_commits`（session commit 適用済み・session 粒度で保守的）
/// と当該 stream の take/segment entity を正本とする。
fn project_reference(
    project: &Project,
    session_id: RecordingSessionId,
    stream_id: CaptureStreamId,
) -> Option<String> {
    let rec = &project.recording;
    // Created/Open session は録音継続中の可能性があり、その media を
    // recovery review で破棄してはならない（Closed/Finalized は
    // entity 参照の有無だけで判定する）。
    if let Some(session) = rec.sessions.get(&session_id) {
        match session.state {
            SessionState::Created | SessionState::Open => {
                return Some(format!("session {session_id} is {:?}", session.state));
            }
            SessionState::Closed | SessionState::Finalized => {}
        }
    }
    if rec.applied_commits.contains_key(&session_id) {
        return Some(format!("applied commit registry entry for {session_id}"));
    }
    if let Some(take) = rec.takes.values().find(|t| t.stream_id == stream_id) {
        return Some(format!("take {} on stream {stream_id}", take.id));
    }
    if let Some(seg) = rec.segments.values().find(|s| s.stream_id == stream_id) {
        return Some(format!("segment {} on stream {stream_id}", seg.id));
    }
    None
}

impl MediaReferenceProbe for ProjectMediaProbe<'_> {
    fn check(&self, candidate: &DiscardCandidate) -> ReferenceCheck {
        let Ok(session_id) = RecordingSessionId::from_tagged(&candidate.session_id) else {
            return ReferenceCheck::Unknown(format!(
                "unparseable session_id {}",
                candidate.session_id
            ));
        };
        let Ok(stream_id) = CaptureStreamId::from_tagged(&candidate.capture_stream_id) else {
            return ReferenceCheck::Unknown(format!(
                "unparseable capture_stream_id {}",
                candidate.capture_stream_id
            ));
        };
        // lane_id は参照判定に使わないが、tagged 形式の破損は
        // manifest 破損の兆候として fail-closed に扱う。
        if LaneId::from_tagged(&candidate.lane_id).is_err() {
            return ReferenceCheck::Unknown(format!("unparseable lane_id {}", candidate.lane_id));
        }

        if let Some(why) = project_reference(self.controller.live(), session_id, stream_id) {
            return ReferenceCheck::Referenced(format!("live project: {why}"));
        }
        // undo/redo 履歴 — 未保存状態も参照源になり得るため、live が
        // clean でも履歴側に参照が残る場合は Discard を拒否する。
        for (i, project) in self.controller.history_states().enumerate() {
            if let Some(why) = project_reference(project, session_id, stream_id) {
                return ReferenceCheck::Referenced(format!("project history[{i}]: {why}"));
            }
        }
        match self.saved_head_project() {
            Ok(Some(project)) => {
                if project.project_id != self.controller.live().project_id {
                    return ReferenceCheck::Unknown(
                        "saved HEAD belongs to a different project".into(),
                    );
                }
                if let Some(why) = project_reference(&project, session_id, stream_id) {
                    return ReferenceCheck::Referenced(format!("saved HEAD: {why}"));
                }
            }
            Ok(None) => {}
            Err(why) => return ReferenceCheck::Unknown(why),
        }
        ReferenceCheck::Clear
    }
}

/// 複数 probe の合成。いずれかが `Referenced` ならそれを返し、
/// `Clear` 以外が複数ある場合は最初の `Unknown` を返す
/// （fail-closed: 判定不能を握り潰さない）。
pub struct CompositeProbe<'a> {
    probes: Vec<&'a dyn MediaReferenceProbe>,
}

impl<'a> CompositeProbe<'a> {
    /// probe 列を合成する。空列は常に `Clear`。
    #[must_use]
    pub fn new(probes: Vec<&'a dyn MediaReferenceProbe>) -> Self {
        Self { probes }
    }
}

impl MediaReferenceProbe for CompositeProbe<'_> {
    fn check(&self, candidate: &DiscardCandidate) -> ReferenceCheck {
        let mut unknown = None;
        for probe in &self.probes {
            match probe.check(candidate) {
                ReferenceCheck::Clear => {}
                ReferenceCheck::Referenced(why) => return ReferenceCheck::Referenced(why),
                ReferenceCheck::Unknown(why) => {
                    unknown.get_or_insert(why);
                }
            }
        }
        unknown.map_or(ReferenceCheck::Clear, ReferenceCheck::Unknown)
    }
}
