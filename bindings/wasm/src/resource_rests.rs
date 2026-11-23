//! Premises of edits to one stored resource, rather than the whole resource library.

use kition_kit::{Covered, CoveredAsk, CoveredAt, CoveredContent, Covers, Premise, Under};
use kition_photo_engine::doc::Document;
use serde_json::{Value, json};

fn selector(list: &str, member: &str, value: &Value) -> Option<String> {
    Some(format!("{list}[{member}={value}]"))
}

pub fn under(doc: &Document, command: &str, p: &Value) -> Option<Under> {
    const NONE: [&str; 0] = [];
    let one = |premise: Premise| Some(Under { premises: vec![premise], ..Default::default() });
    match command {
        // A comp, a saved path and a note are items of the image's lists, addressed by id, name
        // or index: the premise is the item the address named at basis, followed through the
        // image's `~compContents`, `~pathContents` and `~noteContents` ([`resource_covered`]).
        "layerComp.rename" => {
            p.get("comp")?;
            Some(contents_under(&["~compContents"], NONE.to_vec()))
        }
        "path.rename" => {
            let name = p.get("name")?.as_str()?;
            if name.is_empty() || name.eq_ignore_ascii_case("work") || name == "Work Path" || name == "layer" {
                return None;
            }
            // The clipping path follows the rename.
            Some(contents_under(&["~pathContents"], vec!["docState.clipping_path"]))
        }
        "notes.set" => {
            p.get("index")?.as_u64()?;
            Some(contents_under(&["~noteContents"], NONE.to_vec()))
        }
        "view.moveGuide" => {
            p.get("index")?.as_u64()?;
            if !matches!(p.get("orientation")?.as_str()?, "vertical" | "horizontal") {
                return None;
            }
            Some(contents_under(&["guides"], NONE.to_vec()))
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
            let mut premise = Premise::on(crate::place::IMAGE).fields(fields.clone()).positions(NONE);
            for field in fields {
                premise = premise.identity(field, "id");
            }
            one(premise)
        }
        "channel.rename" => Some(Under {
            objects: vec![json!(crate::place::IMAGE)],
            premises: vec![Premise::on(crate::place::IMAGE).fields(NONE).positions(NONE)],
            covers: Some(Covers { fields: vec!["~channelContents".to_owned()] }),
            ..Default::default()
        }),
        _ => None,
    }
}

/// A premise on the image's `fields` (none: only that it exists) whose place the kit tells by
/// being asked about the `covers` fields ([`PhotoDocument::covered_at`]).
fn contents_under(covers: &[&str], fields: Vec<&str>) -> Under {
    Under {
        objects: vec![json!(crate::place::IMAGE)],
        premises: vec![Premise::on(crate::place::IMAGE).fields(fields).positions([] as [&str; 0])],
        covers: Some(Covers { fields: covers.iter().map(|field| (*field).to_owned()).collect() }),
        ..Default::default()
    }
}

/// How a call addresses one item of a stored list: by its place or by a member's value.
pub(crate) enum Address<'a> {
    Index(usize),
    Member(&'a str, &'a Value),
}

impl Address<'_> {
    fn choose<'v>(&self, items: &'v [Value]) -> Option<&'v Value> {
        match self {
            Address::Index(index) => items.get(*index),
            Address::Member(member, value) => items.iter().find(|item| item.get(*member) == Some(*value)),
        }
    }
}

/// Whether the item `address` named at basis (in `before`, else `now`) is still there and what
/// of it changed since: deleted, changed in one of `assigned`, or now at another address (then
/// the place no longer holds it). `None` when it cannot be told (no identity, or a duplicate).
pub(crate) fn resource_covered(address: &Address<'_>, key: &str, assigned: &[&str], derived: &str, asks: &[CoveredAsk], now: &Value) -> Option<CoveredAt> {
    let Some(ask) = asks.iter().find(|ask| ask.id == crate::place::IMAGE) else { return Some(image_untouched(asks)) };
    // Only what the contents were at basis matters (the image's other fields may be unknown).
    if ask.unknown.iter().any(|field| field == derived) {
        return None;
    }
    let before = ask.before.get(derived).unwrap_or(now).as_array()?;
    let now = now.as_array()?;
    let old = address.choose(before)?;
    let identity = old.get(key).filter(|value| value.is_string() || value.is_number())?;
    let count = |items: &[Value]| items.iter().filter(|item| item.get(key) == Some(identity)).count();
    if count(before) != 1 || count(now) > 1 {
        return None;
    }
    let same = now.iter().find(|item| item.get(key) == Some(identity));
    let content = match same {
        None => CoveredContent::Deleted,
        Some(item) if assigned.iter().any(|field| item.get(*field) != old.get(*field)) => CoveredContent::Changed,
        Some(_) => CoveredContent::Same,
    };
    let moved = same.is_some() && address.choose(now).is_none_or(|item| item.get(key) != Some(identity));
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

/// The place an address command (`notes.set`, `layerComp.rename`, `path.rename`,
/// `view.moveGuide`) names, told from the image's stored lists now (`contents`: the derived
/// contents by name, and `guides`, the guides as a list per axis).
pub(crate) fn address_covered(command: &str, p: &Value, asks: &[CoveredAsk], contents: &Value) -> Option<CoveredAt> {
    let index = || p.get("index").and_then(Value::as_u64).and_then(|i| usize::try_from(i).ok());
    match command {
        "notes.set" => {
            let mut assigned: Vec<&str> = ["text", "author", "color"].into_iter().filter(|field| p.get(*field).is_some()).collect();
            if p.get("x").is_some() || p.get("y").is_some() {
                assigned.extend(["position", "popup"]);
            }
            resource_covered(&Address::Index(index()?), "key", &assigned, "~noteContents", asks, contents.get("noteContents")?)
        }
        "layerComp.rename" => {
            let comp = p.get("comp")?;
            let address = if comp.is_u64() { Address::Member("id", comp) } else { Address::Member("name", comp) };
            resource_covered(&address, "id", &["name"], "~compContents", asks, contents.get("compContents")?)
        }
        "path.rename" => resource_covered(&Address::Member("name", p.get("name")?), "key", &["name"], "~pathContents", asks, contents.get("pathContents")?),
        "view.moveGuide" => {
            let axis = p.get("orientation")?.as_str()?;
            let Some(ask) = asks.iter().find(|ask| ask.id == crate::place::IMAGE) else { return Some(image_untouched(asks)) };
            if ask.unknown.iter().any(|field| field == "guides") {
                return None;
            }
            let now_map = contents.get("guides")?;
            let before_map = ask.before.get("guides").unwrap_or(now_map);
            // As lists of `{key, position}`, in order.
            let listed = |map: &Value| -> Option<Value> {
                Some(Value::Array(guide_keys(map, axis)?.into_iter().map(|key| json!({ "key": key, "position": map[key.as_str()]["position"] })).collect()))
            };
            let before = listed(before_map)?;
            let mut asks = asks.to_vec();
            if let Some(ask) = asks.iter_mut().find(|ask| ask.id == crate::place::IMAGE) {
                ask.before.insert("~guides".to_owned(), before);
            }
            resource_covered(&Address::Index(index()?), "key", &["position"], "~guides", &asks, &listed(now_map)?)
        }
        _ => None,
    }
}

/// The answer when others changed nothing of the image (only objects it does not hold, a note
/// they made): the place is as it was, and none of what was asked is in it.
fn image_untouched(asks: &[CoveredAsk]) -> CoveredAt {
    CoveredAt {
        under: vec![json!(crate::place::IMAGE)],
        covered: asks.iter().map(|a| Covered { id: a.id.clone(), covered: Some(false), ..Default::default() }).collect(),
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
    let pattern = stored_pattern(command, p);
    if channels.is_empty() && source.is_none() && pattern.is_none() {
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
    // A stored pattern, by its id or its name: the one it named at basis, its name and pixels.
    if let Some(pattern) = pattern {
        let now = doc.derived_values(&[json!(crate::place::IMAGE)], &["patternContents".to_owned()])?;
        let Some(now) = now.first().and_then(|v| v.values.get("patternContents")).cloned() else { return Ok(None) };
        let value = json!(pattern);
        let before = asks.iter().find(|a| a.id == crate::place::IMAGE).and_then(|a| a.before.get("~patternContents")).unwrap_or(&now);
        let by_key = before.as_array().is_some_and(|items| items.iter().any(|item| item.get("key") == Some(&value)));
        let address = if by_key { Address::Member("key", &value) } else { Address::Member("name", &value) };
        let Some(covered) = resource_covered(&address, "key", &["width", "height", "pixels"], "~patternContents", asks, &now) else { return Ok(None) };
        for entry in covered.covered.into_iter().filter(|a| a.id == crate::place::IMAGE) {
            if let Some(existing) = answer.covered.iter_mut().find(|a| a.id == entry.id) {
                existing.covered = match (existing.covered, entry.covered) {
                    (Some(a), Some(b)) => Some(a || b),
                    _ => None,
                };
                existing.content = merge_content(existing.content, entry.content);
            } else {
                answer.covered.push(entry);
            }
        }
        if covered.under.is_empty()
            && !answer.covered.iter().any(|a| a.id == crate::place::IMAGE && matches!(a.content, Some(CoveredContent::Changed | CoveredContent::Deleted)))
        {
            answer.under.retain(|id| id != crate::place::IMAGE);
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

/// The keys of `axis`'s guides in order, from the `guides` map's value (`null` entries are
/// guides made since).
fn guide_keys(map: &Value, axis: &str) -> Option<Vec<String>> {
    let mut entries: Vec<(f64, &String)> = map
        .as_object()?
        .iter()
        .filter(|(_, entry)| entry.get("axis").and_then(Value::as_str) == Some(axis))
        .map(|(key, entry)| (entry.get("order").and_then(Value::as_f64).unwrap_or(0.0), key))
        .collect();
    entries.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    Some(entries.into_iter().map(|(_, key)| key.clone()).collect())
}

pub(crate) fn channel_covered(address: &Value, assigning: bool, asks: &[CoveredAsk], now: &Value) -> Option<CoveredAt> {
    let Some(ask) = asks.iter().find(|ask| ask.id == crate::place::IMAGE) else { return Some(image_untouched(asks)) };
    // Only the channels at basis matter (the image's other fields may be unknown).
    if ask.unknown.iter().any(|field| field == "~channelContents") {
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
