use super::*;
use kition_kit::session::model::Exchange;

type Holds = BTreeMap<String, (String, bool)>;
type Facts = BTreeSet<String>;

fn parts(path: &str) -> Vec<String> {
    let (mut out, mut start, mut brackets, mut quoted, mut escaped) = (Vec::new(), 0, 0, false, false);
    for (i, c) in path.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && quoted {
            escaped = true;
            continue;
        }
        if c == '"' {
            quoted = !quoted;
        }
        if !quoted {
            match c {
                '[' => brackets += 1,
                ']' => brackets -= 1,
                '.' if brackets == 0 => {
                    out.push(path[start..i].to_owned());
                    start = i + 1;
                }
                _ => {}
            }
        }
    }
    out.push(path[start..].to_owned());
    out
}

fn collect(path: &str, value: &Value, objects: &BTreeMap<String, Value>, out: &mut Facts) {
    if (path.ends_with(".mask") || path.ends_with(".surface"))
        && let Some(object) = value.as_str().and_then(|id| objects.get(id))
    {
        out.insert(format!("{path}.!ref"));
        collect(path, object, objects, out);
        return;
    }
    match value {
        Value::Object(map) if !map.is_empty() => {
            for (key, value) in map {
                if key != "$type" {
                    collect(&format!("{path}.{key}"), value, objects, out);
                }
            }
        }
        Value::Array(items) if !items.is_empty() => {
            for (index, value) in items.iter().enumerate() {
                collect(&format!("{path}.{index}"), value, objects, out);
            }
        }
        _ => {
            out.insert(path.to_owned());
        }
    }
}

fn expand(path: &str, value: &Value, steps: &[String], objects: &BTreeMap<String, Value>, out: &mut Facts) {
    if steps.is_empty() {
        collect(path, value, objects, out);
        return;
    }
    let value = if let Some(object) = value.as_str().and_then(|id| objects.get(id)) {
        out.insert(format!("{path}.!ref"));
        object
    } else {
        value
    };
    let step = &steps[0];
    if step == "*" {
        collect(path, value, objects, out);
        return;
    }
    if let Some((key, test)) = step.split_once('[') {
        let (member, want) = test.trim_end_matches(']').split_once('=').unwrap();
        let want: Value = serde_json::from_str(want).unwrap();
        if let Some((index, item)) = value.get(key).and_then(Value::as_array).and_then(|a| a.iter().enumerate().find(|(_, v)| v.get(member) == Some(&want))) {
            expand(&format!("{path}.{key}.{index}"), item, &steps[1..], objects, out);
        } else {
            out.insert(format!("{path}.{}.!absent", steps.join(".")));
        }
    } else if let Some(next) = value.get(step).or_else(|| step.parse::<usize>().ok().and_then(|i| value.get(i))) {
        expand(&format!("{path}.{step}"), next, &steps[1..], objects, out);
    } else {
        out.insert(format!("{path}.{}", steps.join(".")));
    }
}

fn paths(holds: &Holds, editor: &str, objects: &BTreeMap<String, Value>) -> Facts {
    let mut out = Facts::new();
    for state in editor.split(',').filter(|s| !s.is_empty()) {
        out.insert(format!("editor.{state}"));
    }
    for (id, (fields, at)) in holds {
        out.insert(format!("{id}.!exists"));
        if *at {
            out.insert(format!("{id}.!position"));
        }
        let empty = json!({});
        let value = objects.get(id).unwrap_or(&empty);
        for field in split_fields(fields) {
            expand(id, value, &parts(&field), objects, &mut out);
        }
    }
    out
}

pub(super) fn split_fields(fields: &str) -> Vec<String> {
    // Commas inside JSON selectors and tile coordinates belong to the field.
    let (mut out, mut start, mut depth, mut quote, mut escape) = (Vec::new(), 0, 0, false, false);
    for (i, c) in fields.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if quote && c == '\\' {
            escape = true;
            continue;
        }
        if c == '"' {
            quote = !quote;
        }
        if !quote {
            match c {
                '[' => depth += 1,
                ']' => depth -= 1,
                ',' if depth == 0 => {
                    out.push(fields[start..i].to_owned());
                    start = i + 1;
                }
                _ => {}
            }
        }
    }
    out.push(fields[start..].to_owned());
    out.retain(|f| f != "-" && !f.is_empty());
    out
}

fn replace(holds: &mut Holds, id: &str, from: &str, to: &str) {
    if let Some((fields, _)) = holds.get_mut(id) {
        *fields = split_fields(fields)
            .into_iter()
            .flat_map(|f| if f == from { split_fields(to) } else { vec![f] })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(",");
        if fields.is_empty() {
            *fields = "-".to_owned();
        }
    }
}

fn add(holds: &mut Holds, id: &str, field: &str) {
    let entry = holds.entry(id.to_owned()).or_insert_with(|| ("-".to_owned(), false));
    let mut fields = split_fields(&entry.0);
    fields.push(field.to_owned());
    fields.sort();
    fields.dedup();
    entry.0 = fields.join(",");
}

fn channel_fields(p: &Value, objects: &BTreeMap<String, Value>) -> Vec<String> {
    let Some(channels) = objects.get("image").and_then(|v| v.get("~channelContents")).and_then(Value::as_array) else { return vec![] };
    ["source", "source1", "source2", "mask"]
        .into_iter()
        .filter_map(|source| {
            let channel = p.get(source).and_then(|v| v.get("channel"))?;
            let selected = channel
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| channels.get(i))
                .or_else(|| channels.iter().find(|v| v.get("name") == Some(channel)))?;
            Some(format!("~channelContents[name={}]", selected.get("name")?))
        })
        .collect()
}

/// The key of the image's first vertical guide (its `guides` map's entry of least order).
fn first_vertical_guide(objects: &BTreeMap<String, Value>) -> String {
    objects
        .get("image")
        .and_then(|image| image.get("guides"))
        .and_then(Value::as_object)
        .and_then(|guides| {
            guides
                .iter()
                .filter(|(_, guide)| guide["axis"] == "vertical")
                .min_by(|a, b| a.1["order"].as_f64().unwrap_or(0.0).total_cmp(&b.1["order"].as_f64().unwrap_or(0.0)))
                .map(|(key, _)| key.clone())
        })
        .unwrap()
}

fn reading(row: &Row, p: &Value, objects: &BTreeMap<String, Value>) -> Holds {
    let mut out: Holds = row.holds.iter().map(|(id, f, at)| (id.to_string(), (f.to_string(), *at))).collect();
    // Source refinements of the frozen coarse rows:
    // extra_cmds.rs:432 changes one mask flag; vector_cmds.rs:1090-1165 edits
    // vector_mask alone; layer_multi_cmds.rs:960 reads all link memberships for
    // allocation and singleton cleanup. The rows themselves remain frozen.
    match row.command {
        "layer.layerMask.enabled" | "layer.layerMask.linked" => {
            let flag = if row.command.ends_with("enabled") { "enabled" } else { "linked" };
            for (id, (fields, _)) in out.clone() {
                if id != "image" {
                    out.insert(id, (format!("mask.props.{flag}"), false));
                    let _ = fields;
                }
            }
        }
        _ if row.command.starts_with("layer.vectorMask.") => {
            for id in out.clone().keys().filter(|id| id.as_str() != "image") {
                replace(&mut out, id, "extra", "extra.vector_mask");
            }
        }
        "layer.linkLayers" => {
            for id in out.clone().keys() {
                replace(&mut out, id, "extra", "extra.link_group");
            }
            add(&mut out, "image", "~linkGroups");
        }
        // A historical content question replaces touched-tile and raw canvas-size holds.
        // pixels::fill/clear and brush stroke clipping consume only the destination.
        _ => {}
    }
    let bounded = out.values().any(|(f, _)| split_fields(f).iter().any(|f| f == "tiles@"));
    if bounded {
        replace(&mut out, "image", "width", "width@clip");
        replace(&mut out, "image", "height", "height@clip");
        for (id, (fields, _)) in out.clone() {
            if split_fields(&fields).iter().any(|f| f == "tiles@") {
                add(&mut out, &id, "canvas@coordinates");
            }
        }
    }
    match row.command {
        // edit_cmds.rs::guide_cmd replaces the addressed guide: the `guides` map's entry of the
        // first vertical guide, by its key.
        "view.moveGuide" => {
            replace(&mut out, "image", "guides.vertical.0", &format!("guides.{}.position", first_vertical_guide(objects)));
        }
        "layer.setLabelColor" => {
            replace(&mut out, "image", "~subtreeLabels[id=\"$G\"].labels", "-");
            add(&mut out, "$G", "~subtreeLabels");
            if out.get("image").is_some_and(|v| v.0 == "-") {
                out.remove("image");
            }
        }
        _ => {}
    }
    if matches!(
        row.command,
        "edit.fill" | "paint.bucket" | "paint.patternStamp" | "layer.newFillLayer.pattern" | "layer.layerStyle.patternOverlay" | "brush.texturePattern"
    ) && let Some(pattern) = p.get("pattern").and_then(Value::as_str).filter(|s| !s.is_empty())
    {
        replace(&mut out, "image", "patterns", &format!("~patternContents[name={}]", json!(pattern)));
    }
    if matches!(row.command, "image.applyImage" | "image.calculations") {
        let channels = channel_fields(p, objects);
        if !channels.is_empty() {
            replace(&mut out, "image", "channels", "-");
            replace(&mut out, "image", "~channelContents", &channels.join(","));
        }
    }
    out
}

pub(super) fn compare(doc: &mut PhotoDocument, roles: &[(&str, u64)], rows: &[Row]) {
    let mut exchange = Exchange::new();
    let replica = exchange.replica(doc).unwrap();
    let state = replica.state_json().unwrap();
    let names: BTreeMap<u64, &str> = roles.iter().map(|(r, id)| (*id, *r)).collect();
    let mut objects: BTreeMap<String, Value> = state["objects"].as_object().unwrap().iter().map(|(id, v)| (id.clone(), v.clone())).collect();
    // Layers by their roles; the image's notes, saved paths, comps and patterns by their tool ids
    // (`comp:1`), the fixture's one note and one saved path as `$NOTE` and `$PATH`.
    let mut aliases = BTreeMap::<String, String>::new();
    for (id, value) in objects.clone() {
        let text = |field: &str| value.get(field).map(|v| v.as_str().map_or_else(|| v.to_string(), str::to_owned));
        let tool = match value.get("$type").and_then(Value::as_str) {
            Some("Note") => text("key").map(|key| (format!("note:{key}"), "$NOTE".to_owned())),
            Some("Path") => text("key").map(|key| (format!("path:{key}"), "$PATH".to_owned())),
            Some("Comp") => text("number").map(|n| (format!("comp:{n}"), format!("comp:{n}"))),
            Some("Pattern") => text("key").map(|key| (format!("pattern:{key}"), format!("pattern:{key}"))),
            _ => None,
        };
        if let Some((tool, alias)) = tool {
            objects.insert(alias.clone(), value.clone());
            aliases.insert(tool, alias);
            continue;
        }
        let role = value.get("number").and_then(Value::as_u64).and_then(|n| names.get(&n)).copied().unwrap_or(&id).to_owned();
        objects.insert(role, value);
    }
    for (role, id) in roles.iter().copied().chain(std::iter::once(("image", 0))) {
        let id = if role == "image" { json!("image") } else { json!(id) };
        for derived in doc.derived(&[id], &[]).unwrap() {
            for (name, value) in derived.values {
                objects.get_mut(role).unwrap().as_object_mut().unwrap().insert(format!("~{name}"), value);
            }
        }
    }
    let commands = doc.commands();
    let mut totals = BTreeMap::<String, (usize, usize, usize, usize)>::new();
    let mut precision = BTreeMap::<&str, usize>::new();
    let mut details = Vec::new();
    for row in rows {
        let command = commands.iter().find(|c| c.id == row.command).unwrap();
        let params = with_roles(&serde_json::from_str(row.params).unwrap(), roles);
        let (editor, holds) = held(doc, command, &params, &names);
        let mut holds: Holds = holds.into_iter().map(|(id, held)| (aliases.get(&id).cloned().unwrap_or(id), held)).collect();
        let under = doc.under(row.command, &params).unwrap();
        if let Some(under) = under {
            for p in under.content {
                let id = p.id.as_u64().and_then(|id| names.get(&id)).copied().unwrap_or("image");
                if p.field == "tiles" {
                    add(&mut holds, id, "tiles@");
                }
                if p.field == "~canvasCoordinates" {
                    let target = p.region.as_u64().and_then(|id| names.get(&id)).copied().unwrap();
                    add(&mut holds, target, "canvas@coordinates");
                }
                if matches!(p.field.as_str(), "width" | "height") {
                    add(&mut holds, id, &format!("{}@clip", p.field));
                }
            }
            if row.command == "channel.rename" {
                add(&mut holds, "image", &format!("~channelContents[name={}].name", params["channel"]));
            }
            // The address commands' items, told through the image's derived contents.
            match row.command {
                "layerComp.rename" => {
                    let member = if params["comp"].is_u64() { "id" } else { "name" };
                    add(&mut holds, "image", &format!("~compContents[{member}={}].name", params["comp"]));
                }
                "path.rename" if under.covers.as_ref().is_some_and(|c| c.fields.iter().any(|f| f == "~pathContents")) => {
                    add(&mut holds, "image", &format!("~pathContents[name={}].name", params["name"]));
                }
                "notes.set" => {
                    let index = &params["index"];
                    for (param, fields) in
                        [("text", &["text"][..]), ("author", &["author"]), ("color", &["color"]), ("x", &["position", "popup"]), ("y", &["position", "popup"])]
                    {
                        if params.get(param).is_some() {
                            for field in fields {
                                add(&mut holds, "image", &format!("~noteContents.{index}.{field}"));
                            }
                        }
                    }
                }
                "view.moveGuide" => add(&mut holds, "image", &format!("guides.{}.position", first_vertical_guide(&objects))),
                _ => {}
            }
            if under.covers.as_ref().is_some_and(|c| c.fields.iter().any(|f| f == "~patternContents"))
                && let Some(pattern) = params.get("pattern").and_then(Value::as_str)
            {
                add(&mut holds, "image", &format!("~patternContents[name={}]", json!(pattern)));
            }
            if matches!(row.command, "image.applyImage" | "image.calculations")
                && under.covers.as_ref().is_some_and(|c| c.fields.iter().any(|f| f == "~channelContents"))
            {
                for field in channel_fields(&params, &objects) {
                    add(&mut holds, "image", &field);
                }
            }
        }
        let expected = reading(row, &params, &objects);
        let expected = paths(&expected, row.editor, &objects);
        let actual = paths(&holds, &editor, &objects);
        let less: Vec<_> = expected.difference(&actual).cloned().collect();
        let more: Vec<_> = actual.difference(&expected).cloned().collect();
        let class = row.command.split('.').next().unwrap().to_owned();
        let counts = totals.entry(class).or_default();
        counts.0 += 1;
        counts.1 += usize::from(!less.is_empty() || !more.is_empty());
        counts.2 += usize::from(!less.is_empty());
        counts.3 += usize::from(!more.is_empty());
        if !more.is_empty() {
            let family = if row.command.starts_with("layer.vectorMask.") {
                "vector-mask"
            } else if matches!(row.command, "layer.layerMask.enabled" | "layer.layerMask.linked") {
                "mask-flags"
            } else if row.command == "layer.linkLayers" {
                "links"
            } else {
                "other"
            };
            *precision.entry(family).or_default() += 1;
        }
        if !less.is_empty() || !more.is_empty() {
            details.push(format!("{} {} LESS {less:?} MORE {more:?}", row.command, row.params));
        }
    }
    for (class, counts) in &totals {
        println!("FACTS {class}: rows={} differing={} less={} more={}", counts.0, counts.1, counts.2, counts.3);
    }
    println!("{}", details.join("\n"));
    // Baseline floors are established before narrowing; missing reads never get a floor.
    assert_eq!(totals.values().map(|c| c.2).sum::<usize>(), 0, "held less than the frozen reading");
    for (family, count) in precision {
        let ceiling = 0;
        assert!(count <= ceiling, "{family}: held-more ceiling {count} > {ceiling}");
    }
    for (class, (_, differing, _, more)) in totals {
        let ceiling = 0;
        assert!(differing <= ceiling, "{class}: differing-row ceiling {differing} > {ceiling}");
        assert!(more <= ceiling, "{class}: held-more ceiling {more} > {ceiling}");
    }
}
