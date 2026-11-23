//! A type layer's characters: a `TextBody` object (`Layer.text`) whose `content` is a KDM text,
//! so two people typing in one type layer both keep what they typed. The engine's style runs
//! ([`TextRun`], [`ParagraphRun`], lengths in UTF-8 bytes) are marks on the characters: `char`
//! (the character style) and `paragraph` (the paragraph style), each with the run's index
//! (`run`, `paragraphRun`), so two neighbouring runs of one style stay two runs.

use kition_kit::kdm::{KdmError, ObjectId, Replica, Tx};
use kition_photo_doc::text::{CharStyle, ParagraphRun, ParagraphStyle, TextRun};
use serde_json::{Map, Value, json};

use crate::kdm_err;
use crate::lists::splice_text;

type KdmResult<T> = Result<T, KdmError>;

const FIELD: &str = "content";

/// The text of one type layer as stored: characters and their runs.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Body {
    pub text: String,
    pub runs: Vec<TextRun>,
    pub paragraphs: Vec<ParagraphRun>,
}

/// One character's marks: (character run, its style) and (paragraph run, its style), each
/// `None` past the runs' end.
type CharMarks = (Option<(usize, Value)>, Option<(usize, Value)>);

/// Each character's run index and style, from runs measured in UTF-8 bytes. A run boundary
/// inside a character counts at the character's start.
fn per_char<S: serde::Serialize>(text: &str, runs: impl Iterator<Item = (usize, S)>) -> KdmResult<Vec<Option<(usize, Value)>>> {
    let mut bounds: Vec<(usize, usize, Value)> = Vec::new();
    let mut start = 0usize;
    for (len, style) in runs {
        let end = start.saturating_add(len);
        bounds.push((start, end, serde_json::to_value(style).map_err(kdm_err)?));
        start = end;
    }
    let mut out = Vec::with_capacity(text.chars().count());
    let mut run = 0usize;
    for (byte, _) in text.char_indices() {
        while bounds.get(run).is_some_and(|(_, end, _)| byte >= *end) {
            run += 1;
        }
        out.push(bounds.get(run).filter(|(start, _, _)| byte >= *start).map(|(_, _, style)| (run, style.clone())));
    }
    Ok(out)
}

fn marks_of(body: &Body) -> KdmResult<Vec<CharMarks>> {
    let chars = per_char(&body.text, body.runs.iter().map(|run| (run.len, &run.style)))?;
    let paragraphs = per_char(&body.text, body.paragraphs.iter().map(|run| (run.len, &run.style)))?;
    Ok(chars.into_iter().zip(paragraphs).collect())
}

/// Sets (or removes) one mark pair over the characters `start..end`.
fn mark(tx: &mut Tx<'_>, object: &ObjectId, start: usize, end: usize, keys: (&str, &str), value: &Option<(usize, Value)>) -> KdmResult<()> {
    match value {
        Some((index, style)) => {
            tx.text_mark(object, FIELD, start, end, keys.0, json!(index))?;
            tx.text_mark(object, FIELD, start, end, keys.1, style.clone())?;
        }
        None => {
            tx.text_unmark(object, FIELD, start, end, keys.0)?;
            tx.text_unmark(object, FIELD, start, end, keys.1)?;
        }
    }
    Ok(())
}

/// Marks the characters whose wanted marks (`after`) differ from what they hold (`had`: `None`
/// for a character whose marks are not known, one just typed), in ranges of one value.
fn remark(tx: &mut Tx<'_>, object: &ObjectId, after: &[CharMarks], had: &[Option<CharMarks>]) -> KdmResult<()> {
    for (first, keys) in [(true, ("run", "char")), (false, ("paragraphRun", "paragraph"))] {
        let pick = |marks: &CharMarks| if first { marks.0.clone() } else { marks.1.clone() };
        let wanted = |i: usize| after.get(i).and_then(pick);
        let known = |i: usize| had.get(i).cloned().flatten().map(|marks| pick(&marks));
        let mut i = 0usize;
        while i < after.len() {
            let value = wanted(i);
            if known(i).as_ref() == Some(&value) {
                i += 1;
                continue;
            }
            let start = i;
            // Whether a character of the range may hold a mark now.
            let mut marked = false;
            while i < after.len() && wanted(i) == value && known(i).as_ref() != Some(&value) {
                marked |= known(i).is_none_or(|held| held.is_some());
                i += 1;
            }
            if value.is_some() || marked {
                mark(tx, object, start, i, keys, &value)?;
            }
        }
    }
    Ok(())
}

/// Creates a `TextBody` holding `body`.
pub(crate) fn create(tx: &mut Tx<'_>, body: &Body) -> KdmResult<ObjectId> {
    let object = tx.create("TextBody", Map::new())?;
    if !body.text.is_empty() {
        tx.text_insert(&object, FIELD, 0, &body.text)?;
        let after = marks_of(body)?;
        // New characters hold no marks.
        remark(tx, &object, &after, &vec![Some((None, None)); after.len()])?;
    }
    Ok(object)
}

/// Writes what differs between `before` (what `object` holds) and `after`: the characters
/// between their common start and end, and the marks of the characters whose runs differ.
pub(crate) fn update(tx: &mut Tx<'_>, object: &ObjectId, before: &Body, after: &Body) -> KdmResult<bool> {
    if before == after {
        return Ok(false);
    }
    let old: Vec<char> = before.text.chars().collect();
    let new: Vec<char> = after.text.chars().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..].iter().rev().zip(new[prefix..].iter().rev()).take_while(|(a, b)| a == b).count();
    splice_text(tx, object, FIELD, &before.text, &after.text)?;
    let had_marks = marks_of(before)?;
    let after_marks = marks_of(after)?;
    // What each character of `after` held before: the common start and end kept their marks,
    // the characters typed between take their neighbour's (`expand: after`), which is never
    // trusted here.
    let had: Vec<Option<CharMarks>> = (0..new.len())
        .map(|i| {
            if i < prefix {
                had_marks.get(i).cloned()
            } else if i >= new.len() - suffix {
                had_marks.get(i + old.len() - new.len()).cloned()
            } else {
                None
            }
        })
        .collect();
    let len = tx.text_len(object, FIELD)?;
    if len == after_marks.len() {
        remark(tx, object, &after_marks, &had)?;
    }
    Ok(true)
}

/// Reads a `TextBody`'s characters and runs.
pub(crate) fn read(replica: &Replica, object: &ObjectId) -> KdmResult<Body> {
    let pieces = replica.text_delta(object, FIELD)?;
    let mut body = Body::default();
    let mut char_key: Option<(Value, Value)> = None;
    let mut para_key: Option<(Value, Value)> = None;
    // Unmarked characters after a marked run belong to it, unless nothing marked follows.
    let mut char_pending = 0usize;
    let mut para_pending = 0usize;
    for piece in pieces {
        let bytes = piece.insert.len();
        body.text.push_str(&piece.insert);
        let attrs = &piece.attributes;
        match attrs.get("char") {
            Some(style) => {
                let key = (attrs.get("run").cloned().unwrap_or(Value::Null), style.clone());
                if char_key.as_ref() == Some(&key)
                    && let Some(last) = body.runs.last_mut()
                {
                    last.len += char_pending + bytes;
                } else {
                    if let Some(last) = body.runs.last_mut() {
                        last.len += char_pending;
                    }
                    let style: CharStyle = serde_json::from_value(style.clone()).map_err(kdm_err)?;
                    let len = if body.runs.is_empty() { char_pending + bytes } else { bytes };
                    body.runs.push(TextRun { len, style });
                    char_key = Some(key);
                }
                char_pending = 0;
            }
            None => char_pending += bytes,
        }
        match attrs.get("paragraph") {
            Some(style) => {
                let key = (attrs.get("paragraphRun").cloned().unwrap_or(Value::Null), style.clone());
                if para_key.as_ref() == Some(&key)
                    && let Some(last) = body.paragraphs.last_mut()
                {
                    last.len += para_pending + bytes;
                } else {
                    if let Some(last) = body.paragraphs.last_mut() {
                        last.len += para_pending;
                    }
                    let style: ParagraphStyle = serde_json::from_value(style.clone()).map_err(kdm_err)?;
                    let len = if body.paragraphs.is_empty() { para_pending + bytes } else { bytes };
                    body.paragraphs.push(ParagraphRun { len, style });
                    para_key = Some(key);
                }
                para_pending = 0;
            }
            None => para_pending += bytes,
        }
    }
    Ok(body)
}
