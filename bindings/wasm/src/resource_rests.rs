//! Premises of edits to one stored resource, rather than the whole resource library.

use kition_kit::{ContentPremise, Covered, CoveredAsk, CoveredAt, CoveredContent, Covers, Premise, Under};
use kition_photo_engine::doc::Document;
use serde_json::{Value, json};

fn selector(list: &str, member: &str, value: &Value) -> Option<String> {
    Some(format!("{list}[{member}={value}]"))
}

pub fn under(doc: &Document, command: &str, p: &Value) -> Option<Under> {
    let fields = match command {
        "layerComp.rename" => {
            let comp = p.get("comp")?;
            let member = if comp.is_u64() { "id" } else { "name" };
            vec![format!("{}.name", selector("docState.layer_comps", member, comp)?)]
        }
        "path.rename" => {
            let name = p.get("name")?.as_str()?;
            if name.is_empty() || name.eq_ignore_ascii_case("work") || name == "Work Path" || name == "layer" {
                return None;
            }
            vec![
                format!("{}.name", selector("docState.paths", "name", &json!(name))?),
                format!("{}.name", selector("docState.paths", "name", p.get("to")?)?),
                "docState.clipping_path".to_owned(),
            ]
        }
        "notes.set" => {
            let index = p.get("index")?.as_u64()?;
            let mut fields: Vec<_> = ["text", "author", "color", "open"]
                .into_iter()
                .filter(|key| p.get(*key).is_some())
                .map(|key| format!("docState.notes.{index}.{key}"))
                .collect();
            if p.get("text").is_some() || p.get("author").is_some() {
                fields.push(format!("docState.notes.{index}.modified"));
            }
            if p.get("x").is_some() || p.get("y").is_some() {
                fields.extend([format!("docState.notes.{index}.position"), format!("docState.notes.{index}.popup")]);
            }
            fields
        }
        "view.moveGuide" => {
            let index = p.get("index")?.as_u64()?;
            let orientation = match p.get("orientation")?.as_str()? {
                "vertical" => "vertical",
                "horizontal" => "horizontal",
                _ => return None,
            };
            vec![format!("guides.{orientation}.{index}.position")]
        }
        "slice.set" => {
            let id = p.get("slice")?.as_u64()?;
            let slice = doc.slices.list.iter().find(|slice| u64::from(slice.id) == id);
            if slice.is_some_and(|slice| slice.origin != kition_photo_engine::doc::SliceOrigin::User) {
                return None;
            }
            let root = selector("docState.slices.list", "id", &json!(id))?;
            let mut fields = vec![format!("{root}.id"), format!("{root}.origin")];
            for key in [
                "name",
                "kind",
                "url",
                "target",
                "message",
                "alt",
                "cellText",
                "cellTextIsHtml",
                "horizontalAlign",
                "verticalAlign",
                "background",
                "outsets",
                "rect",
            ] {
                if p.get(key).is_some() {
                    fields.push(format!("{root}.{key}"));
                }
            }
            fields
        }
        "channel.rename" => {
            return Some(Under {
                objects: vec![json!(crate::place::IMAGE)],
                premises: vec![Premise::on(crate::place::IMAGE).fields([] as [&str; 0]).positions([] as [&str; 0])],
                covers: Some(Covers { fields: vec!["~channelContents".to_owned()] }),
                ..Default::default()
            });
        }
        _ => return None,
    };
    let mut premise = Premise::on(crate::place::IMAGE).fields(fields.clone()).positions([] as [&str; 0]);
    for field in fields {
        if command == "path.rename" && field == "docState.clipping_path" {
            continue;
        }
        let key = if matches!(command, "layerComp.rename" | "slice.set") { "id" } else { "key" };
        premise = premise.identity(field, key);
    }
    let content = match command {
        "path.rename" => vec![ContentPremise {
            id: json!(crate::place::IMAGE),
            field: "docState".to_owned(),
            region: json!({"resource":"paths","member":"name","value":p.get("name")?,"key":"key","assigned":["name"]}),
        }],
        "layerComp.rename" => vec![ContentPremise {
            id: json!(crate::place::IMAGE),
            field: "docState".to_owned(),
            region: json!({"resource":"layer_comps","member":if p.get("comp")?.is_u64(){"id"}else{"name"},"value":p.get("comp")?,"key":"id","assigned":["name"]}),
        }],
        _ => vec![],
    };
    let covers = match command {
        "notes.set" => Some("docState.notes"),
        "view.moveGuide" => fields_for_guide(p),
        _ => None,
    };
    Some(Under {
        premises: vec![premise],
        content,
        objects: covers.map(|_| vec![json!(crate::place::IMAGE)]).unwrap_or_default(),
        covers: covers.map(|field| Covers { fields: vec![field.to_owned()] }),
        ..Default::default()
    })
}

fn fields_for_guide(p: &Value) -> Option<&'static str> {
    match p.get("orientation")?.as_str()? {
        "vertical" => Some("guides.vertical"),
        "horizontal" => Some("guides.horizontal"),
        _ => None,
    }
}

pub(crate) fn alpha_address(value: &Value) -> bool {
    value.is_u64()
        || value.as_str().is_some_and(|s| {
            !matches!(
                s.to_ascii_lowercase().as_str(),
                "red"
                    | "green"
                    | "blue"
                    | "rgb"
                    | "cmyk"
                    | "gray"
                    | "grayscale"
                    | "cyan"
                    | "magenta"
                    | "yellow"
                    | "black"
                    | "lab"
                    | "lightness"
                    | "a"
                    | "b"
                    | "merged"
                    | "composite"
                    | "selection"
                    | "quickmask"
                    | "quick mask"
                    | "transparency"
                    | "mask"
                    | "layermask"
                    | "vectormask"
                    | "vector mask"
            )
        })
}

pub(crate) fn source_channels(command: &str, p: &Value) -> Vec<Value> {
    let mut sources = Vec::new();
    if crate::acts::reads_of(command).region
        && let Some(channel) = p.get("region").and_then(|r| r.get("channel"))
    {
        sources.push(channel.clone());
    }
    let keys: &[&str] = match command {
        "image.applyImage" => &["source", "mask"],
        "image.calculations" => &["source1", "source2", "mask"],
        _ => &[],
    };
    for key in keys {
        if let Some(channel) = crate::place::source_spec(p, key, key).get("channel").filter(|v| alpha_address(v)) {
            sources.push(channel.clone());
        }
    }
    sources
}

pub(crate) fn stored_pattern<'a>(command: &str, p: &'a Value) -> Option<&'a str> {
    matches!(
        command,
        "edit.fill" | "paint.bucket" | "paint.patternStamp" | "layer.newFillLayer.pattern" | "layer.layerStyle.patternOverlay" | "brush.texturePattern"
    )
    .then(|| p.get("pattern").and_then(Value::as_str).filter(|s| !s.is_empty()))
    .flatten()
}

pub(crate) fn sources_covered(
    doc: &crate::PhotoDocument,
    command: &str,
    p: &Value,
    asks: &[CoveredAsk],
    answer: Option<CoveredAt>,
) -> Result<Option<CoveredAt>, String> {
    let channels = source_channels(command, p);
    let source = crate::acts::reads_of(command).region.then(|| p.get("region").and_then(|r| r.get("layer"))).flatten();
    if channels.is_empty() && source.is_none() {
        return Ok(answer);
    }
    let mut answer = answer.unwrap_or_else(|| CoveredAt {
        under: doc.under_call(command, p).ok().flatten().map(|u| u.objects).unwrap_or_default(),
        covered: asks.iter().map(|a| Covered { id: a.id.clone(), covered: Some(false), ..Default::default() }).collect(),
    });
    if let Some(source) = source {
        let now = doc.derived_values(std::slice::from_ref(source), &["bounds".to_owned()])?;
        let now = now.first().and_then(|v| v.values.get("bounds"));
        if let Some(ask) = asks.iter().find(|a| crate::context::layer_of(&a.id) == crate::context::layer_of(source)) {
            let before = ask.before.get("~bounds").or(now);
            if ask.deleted || before != now {
                answer.under.retain(|id| crate::context::layer_of(id) != crate::context::layer_of(source));
            }
            if let Some(entry) = answer.covered.iter_mut().find(|a| a.id == ask.id) {
                entry.covered = if ask.unknown.iter().any(|f| f == "~bounds") { None } else { Some(true) };
                entry.gone = ask.deleted;
            }
        }
    }
    if !channels.is_empty() {
        let now = doc.derived_values(&[json!(crate::place::IMAGE)], &["channelContents".to_owned()])?;
        if let Some(now) = now.first().and_then(|v| v.values.get("channelContents")) {
            let mut moved = false;
            for channel in channels {
                if let Some(covered) = channel_covered(&channel, false, asks, now) {
                    moved |= covered.under.is_empty();
                    for entry in covered.covered.into_iter().filter(|a| a.id == crate::place::IMAGE) {
                        if let Some(existing) = answer.covered.iter_mut().find(|a| a.id == entry.id) {
                            existing.covered = match (existing.covered, entry.covered) {
                                (Some(a), Some(b)) => Some(a || b),
                                _ => None,
                            };
                            existing.gone |= entry.gone;
                            existing.content = merge_content(existing.content, entry.content);
                        } else {
                            answer.covered.push(entry);
                        }
                    }
                } else {
                    return Ok(None);
                }
            }
            if moved
                && !answer.covered.iter().any(|a| a.id == crate::place::IMAGE && matches!(a.content, Some(CoveredContent::Changed | CoveredContent::Deleted)))
            {
                answer.under.retain(|id| id != crate::place::IMAGE);
            }
        }
    }
    Ok(Some(answer))
}

fn merge_content(a: Option<CoveredContent>, b: Option<CoveredContent>) -> Option<CoveredContent> {
    use CoveredContent::{Changed, Deleted, Same, Unknown};
    match (a, b) {
        (Some(Deleted), _) | (_, Some(Deleted)) => Some(Deleted),
        (Some(Changed), _) | (_, Some(Changed)) => Some(Changed),
        (Some(Unknown), _) | (_, Some(Unknown)) => Some(Unknown),
        (Some(Same), _) | (_, Some(Same)) => Some(Same),
        _ => None,
    }
}

pub(crate) fn indexed_covered(command: &str, p: &Value, asks: &[CoveredAsk], state: &Value) -> Option<CoveredAt> {
    let field = if command == "notes.set" { "docState.notes" } else { fields_for_guide(p)? };
    let ask = asks.iter().find(|ask| ask.id == crate::place::IMAGE)?;
    if !ask.unknown.is_empty() {
        return None;
    }
    let now = field.split('.').try_fold(state, |value, key| value.get(key))?.as_array()?;
    let historical = Value::Object(ask.before.clone());
    let before = field.split('.').try_fold(&historical, |value, key| value.get(key)).and_then(Value::as_array).unwrap_or(now);
    let old = before.get(usize::try_from(p.get("index")?.as_u64()?).ok()?)?;
    let key = old.get("key")?.as_str()?;
    if before.iter().filter(|item| item.get("key").and_then(Value::as_str) == Some(key)).count() != 1
        || now.iter().filter(|item| item.get("key").and_then(Value::as_str) == Some(key)).count() > 1
    {
        return None;
    }
    let deleted = !now.iter().any(|item| item.get("key").and_then(Value::as_str) == Some(key));
    Some(CoveredAt {
        under: vec![json!(crate::place::IMAGE)],
        covered: asks
            .iter()
            .map(|a| Covered {
                id: a.id.clone(),
                covered: Some(a.id == crate::place::IMAGE),
                content: (a.id == crate::place::IMAGE).then_some(if deleted { CoveredContent::Deleted } else { CoveredContent::Same }),
                ..Default::default()
            })
            .collect(),
    })
}

pub(crate) fn assigned_changed(before: &Value, now: &Value, region: &Value) -> Option<bool> {
    let list = region.get("resource")?.as_str()?;
    let member = region.get("member")?.as_str()?;
    let value = region.get("value")?;
    let key = region.get("key")?.as_str()?;
    let old = before.get(list)?.as_array()?.iter().find(|item| item.get(member) == Some(value))?;
    let identity = old.get(key)?.as_str().map(|s| json!(s)).or_else(|| old.get(key).filter(|v| v.is_number()).cloned())?;
    let current = now.get(list)?.as_array()?.iter().find(|item| item.get(key) == Some(&identity));
    let Some(current) = current else { return Some(false) };
    Some(region.get("assigned")?.as_array()?.iter().filter_map(Value::as_str).any(|field| old.get(field) != current.get(field)))
}

pub(crate) fn channel_covered(address: &Value, assigning: bool, asks: &[CoveredAsk], now: &Value) -> Option<CoveredAt> {
    let ask = asks.iter().find(|ask| ask.id == crate::place::IMAGE)?;
    if !ask.unknown.is_empty() {
        return None;
    }
    let before = ask.before.get("~channelContents").unwrap_or(now).as_array()?;
    let choose = |items: &[Value]| {
        address
            .as_u64()
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| items.get(i))
            .or_else(|| items.iter().find(|c| c.get("name") == Some(address)))
            .cloned()
    };
    let old = choose(before)?;
    let key = old.get("key")?.as_str()?;
    if before.iter().filter(|c| c.get("key").and_then(Value::as_str) == Some(key)).count() != 1 {
        return None;
    }
    let now = now.as_array()?;
    if now.iter().filter(|c| c.get("key").and_then(Value::as_str) == Some(key)).count() > 1 {
        return None;
    }
    let same = now.iter().find(|c| c.get("key").and_then(Value::as_str) == Some(key));
    let changed = if assigning {
        same.and_then(|c| c.get("name")) != old.get("name")
    } else {
        same.is_some_and(|c| ["format", "default", "tiles"].iter().any(|f| c.get(*f) != old.get(*f)))
    };
    let content = if same.is_none() {
        CoveredContent::Deleted
    } else if changed {
        CoveredContent::Changed
    } else {
        CoveredContent::Same
    };
    let moved = same.is_some() && choose(now).is_none_or(|c| c.get("key").and_then(Value::as_str) != Some(key));
    Some(CoveredAt {
        under: if moved && content != CoveredContent::Changed { vec![] } else { vec![json!(crate::place::IMAGE)] },
        covered: asks
            .iter()
            .map(|a| Covered {
                id: a.id.clone(),
                covered: Some(a.id == crate::place::IMAGE),
                content: (a.id == crate::place::IMAGE).then_some(content),
                ..Default::default()
            })
            .collect(),
    })
}
