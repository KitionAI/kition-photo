//! SDK 1935d52 reproductions: numeric limits inside JSON and on command parameters.
use kition_kit::{KitDocument, kdm::Schema, session::model::Exchange};
use kition_photo_kit::PhotoDocument;
use serde_json::json;

fn main() -> Result<(), String> {
    for field in [json!({"kind":"json","minimum":0,"maximum":1}), json!({"kind":"json","properties":{"density":{"kind":"number","minimum":0,"maximum":1}}})] {
        let schema = json!({"kdm":1,"kit":"kit.probe","schemaVersion":1,"root":"Mask",
            "types":{"Mask":{"fields":{"data":field}}}});
        println!("{}", json!({"field":field,"error":Schema::parse(&schema.to_string()).err().map(|error|error.to_string())}));
    }
    let schema = json!({"opacity":{"type":"number","minimum":0,"maximum":1}});
    println!("{}", json!({"commandSchema":schema,"error":kition_kit::schema::validate(&schema).err()}));
    let mut doc = PhotoDocument::create("art.kphoto", 32, 24)?;
    doc.execute("layer.vectorMask.revealAll", json!({"layer":1}))?;
    let mut exchange = Exchange::new();
    let result = exchange.batch(&mut doc, &json!({"ops":[{"op":"set","target":"1","field":"extra.vector_mask.density","value":2}]}), false);
    println!(
        "{}",
        json!({"batchApplied":result.is_ok(),"error":result.err().map(|error|error.to_string()),
        "mask":doc.execute("layer.vectorMask.info", json!({"layer":1}))?})
    );
    Ok(())
}
