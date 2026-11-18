use std::sync::Arc;

use serde_json::json;

use crate::{Origin, Session};

#[test]
fn c8_late_writer_agent_cannot_undo_a_snapshot_over_a_colleague() {
    let mut s = Session::new();
    s.execute("file.new", json!({"width":24,"height":16})).unwrap();
    // The unsupported/reset-replica path keeps local snapshot history.
    let st = s.active_mut().unwrap();
    st.kdm = None;
    st.kdm_blocked = true;
    let agent = Origin::agent("C8-photo-3");
    s.execute_as("layer.new.layer", json!({"name":"Mine"}), &agent).unwrap();
    assert!(s.active().unwrap().kdm().is_none());
    let st = s.active_mut().unwrap();
    Arc::make_mut(&mut st.doc).layers[0].name = "Colleague".into();
    let before = st.doc.clone();
    let result = s.execute_as("edit.undo", json!({}), &agent);
    assert!(result.is_err(), "snapshot undo was allowed to roll back a collaborator: {result:?}");
    assert!(Arc::ptr_eq(&s.active().unwrap().doc, &before), "a refused undo changed the document");
}

#[test]
fn c8_late_writer_actions_cannot_hide_journal_replay() {
    for command in ["filter.lastFilter", "edit.transform.again", "edit.transform.againCopy"] {
        let mut s = Session::new();
        s.execute("file.new", json!({"width":24,"height":16})).unwrap();
        s.execute("layer.new.layer", json!({})).unwrap();
        s.execute("edit.fill", json!({"color":"#ff0000"})).unwrap();
        s.execute("filter.blur.gaussianBlur", json!({"radius":2})).unwrap();
        s.execute("edit.transform", json!({"matrix":[1,0,0,1,4,0]})).unwrap();
        s.actions.list.push(crate::actions_cmds::Action { name: "Replay".into(), steps: vec![(command.into(), json!({}))] });
        let before = s.active().unwrap().doc.clone();
        let answer = s.execute_as("actions.play", json!({"action":"Replay"}), &Origin::agent("C8-photo-3")).unwrap();
        assert_eq!(answer["ran"], 0, "{command}: {answer}");
        assert!(answer["failed"].to_string().contains("premise guard"), "{command}: {answer}");
        assert!(Arc::ptr_eq(&s.active().unwrap().doc, &before));
    }
}
