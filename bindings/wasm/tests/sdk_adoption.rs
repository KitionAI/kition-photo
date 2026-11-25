use kition_kit::KitDocument;
use kition_photo_kit::PhotoDocument;
use serde_json::json;

#[test]
fn reread_keeps_active_selection_and_agent_history() {
    let mut live = PhotoDocument::create("reread.kphoto", 32, 32).unwrap();
    let layer = live.execute("layer.new.layer", json!({"name":"Target"})).unwrap()["layer"].clone();
    live.execute("layer.select", json!({"layer":layer})).unwrap();
    live.mark_saved();
    let mut fresh = PhotoDocument::open("reread.kphoto", &live.save_sealed(None).unwrap()).unwrap();
    fresh.execute("note.add", json!({"text":"outside","x":1,"y":2})).unwrap();
    fresh.carry_replica_objects(&live).unwrap();
    fresh.restore_editor_state(&live.editor_state()).unwrap();
    assert!(live.reconcile_reread(&fresh).unwrap());
    assert_eq!(live.inspect().unwrap()["activeLayer"], layer);
    assert_eq!(live.inspect().unwrap()["notes"][0]["text"], "outside");
    assert!(live.can_undo());
    assert!(live.undo().unwrap().0);
    assert_eq!(live.inspect().unwrap()["notes"][0]["text"], "outside");
    assert!(!live.inspect().unwrap()["layers"].as_array().unwrap().iter().any(|l| l["id"] == layer));
}

#[test]
fn reread_deleted_active_keeps_the_missing_target_identity() {
    let mut live = PhotoDocument::create("deleted.kphoto", 32, 32).unwrap();
    let layer = live.execute("layer.new.layer", json!({"name":"Target"})).unwrap()["layer"].clone();
    let mut fresh = PhotoDocument::open("deleted.kphoto", &live.save_sealed(None).unwrap()).unwrap();
    fresh.execute("layer.delete", json!({"layer":layer})).unwrap();
    fresh.carry_replica_objects(&live).unwrap();
    fresh.restore_editor_state(&live.editor_state()).unwrap();
    live.reconcile_reread(&fresh).unwrap();
    assert_eq!(live.editor_state()["active"], layer);
    let under = live.under("layer.setProps", &json!({"opacity":0.5})).unwrap().unwrap();
    assert!(under.premises.iter().any(|p| p.id == layer));
    assert!(live.execute("layer.setProps", json!({"opacity":0.5})).is_err());
}

#[test]
fn premise_fork_keeps_editor_state_and_is_isolated() {
    let mut live = PhotoDocument::create("fork.kphoto", 32, 32).unwrap();
    let layer = live.execute("layer.new.layer", json!({"name":"Target"})).unwrap()["layer"].clone();
    live.execute("select.rect", json!({"x":2,"y":3,"width":4,"height":5})).unwrap();
    let before = live.inspect().unwrap();
    let mut fork = live.fork_for_premises().unwrap();
    fork.restore_editor_state(&live.editor_state()).unwrap();
    assert_eq!(fork.inspect().unwrap()["activeLayer"], layer);
    assert_eq!(fork.inspect().unwrap()["selectionBounds"], before["selectionBounds"]);
    fork.execute("layer.setProps", json!({"layer":layer,"name":"Planned"})).unwrap();
    assert_eq!(live.inspect().unwrap(), before);
}

#[test]
fn bounded_pixel_writes_declare_historical_content_and_effective_clipping() {
    let doc = PhotoDocument::create("region.kphoto", 32, 32).unwrap();
    let layer = doc.inspect().unwrap()["activeLayer"].clone();
    let under = doc.under("edit.fill", &json!({"layer":layer,"color":"#123456","region":{"rect":[2,3,4,5]}})).unwrap().unwrap();
    assert!(under.content.iter().any(|p| p.id == layer && p.field == "tiles"));
    for field in ["width", "height"] {
        assert!(under.content.iter().any(|p| p.id == "image" && p.field == field));
        assert!(!under.premises.iter().filter(|p| p.id == "image").flat_map(|p| p.fields.iter().flatten()).any(|f| f == field));
    }
}
