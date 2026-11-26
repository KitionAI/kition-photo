//! C8-expectations review photo: every supported slice assignment holds its observed field.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::*;
use kition_kit::KitDocument;
use kition_kits::tools::CallOptions;
use kition_photo_kit::PhotoDocument;
use serde_json::{Value, json};

const FILE: &str = "SliceExpectations.kphoto";

fn saved(bytes: &[u8]) -> Value {
    let zip = kition_photo_format::zip::ZipReader::new(bytes).unwrap();
    let manifest: Value = serde_json::from_slice(&zip.read_by_name("manifest.json", 16 * 1024 * 1024).unwrap()).unwrap();
    let mut doc = manifest["document"].clone();
    doc.as_object_mut().unwrap().remove("id");
    doc.as_object_mut().unwrap().remove("selection");
    doc
}

#[tokio::test(flavor = "multi_thread")]
async fn slice_same_field_matrix() {
    let mut doc = PhotoDocument::create(FILE, 96, 64).unwrap();
    doc.execute("slice.new", json!({"rect":[8,8,16,16],"name":"Observed"})).unwrap();
    let bytes = doc.save(Some(FILE)).unwrap();
    let mut wrong = vec![];
    let (mut applied, mut necessary) = (0, 0);
    for (field, mine, other) in [
        ("name", json!("Mine"), json!("Colleague")),
        ("kind", json!("noImage"), json!("table")),
        ("url", json!("Mine"), json!("Colleague")),
        ("target", json!("Mine"), json!("Colleague")),
        ("message", json!("Mine"), json!("Colleague")),
        ("alt", json!("Mine"), json!("Colleague")),
        ("cellText", json!("Mine"), json!("Colleague")),
        ("cellTextIsHtml", json!(true), json!(true)),
        ("horizontalAlign", json!(1), json!(2)),
        ("verticalAlign", json!(1), json!(2)),
        ("background", json!("#00ff00"), json!("#ff0000")),
        ("outsets", json!([1, 0, 0, 0]), json!([2, 0, 0, 0])),
        ("rect", json!([12, 8, 16, 16]), json!([16, 8, 16, 16])),
    ] {
        for edit in ["different value", "same final value", "other field"] {
            let value = if field == "cellTextIsHtml" && edit == "different value" { json!(false) } else { mine.clone() };
            let params = json!({"slice":1, (field):value});
            let change = match edit {
                "other field" if field == "url" => json!({"slice":1,"name":"Colleague"}),
                "other field" => json!({"slice":1,"url":"Colleague"}),
                "same final value" => params.clone(),
                _ => json!({"slice":1, (field):other}),
            };
            let f = Fixture::from_bytes(FILE, &bytes, CallOptions::default());
            let (_, basis) = f.inspect().await;
            f.colleague(&[("slice.set", change)]);
            let before = std::fs::read(f.file()).unwrap();
            let result = f.call("run", json!({"basis":basis,"steps":[{"command":"slice.set","params":params}]})).await;
            let label = format!("slice.set/{field}/{edit}");
            // same-field decision: even assigning the colleague's new value refuses.
            if edit != "other field" {
                if let Err(error) = result {
                    assert_eq!(error["error"]["code"], "conflict", "{label}: {error}");
                    assert!(error["conflicts"].as_array().unwrap().iter().any(|c| c["reason"] == "changed"), "{label}: {error}");
                    assert_eq!(std::fs::read(f.file()).unwrap(), before, "{label}");
                    necessary += 1;
                } else {
                    wrong.push(label);
                }
            } else {
                result.unwrap_or_else(|error| panic!("{label}: {error}"));
                let mut reference = PhotoDocument::open(FILE, &before).unwrap();
                reference.execute("slice.set", params).unwrap();
                let expected = saved(&reference.save(Some(FILE)).unwrap());
                let actual = saved(&std::fs::read(f.file()).unwrap());
                assert_eq!(actual, expected, "{label}: saved merge");
                applied += 1;
            }
            f.shutdown().await;
        }
    }
    println!("SLICE cells=39 applied={applied} necessary={necessary} wrong={} {wrong:?}", wrong.len());
    assert!(wrong.is_empty(), "same-field overwrites: {wrong:?}");
    assert_eq!((applied, necessary), (13, 26));
}
