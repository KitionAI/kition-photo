//! Private PSD resource identities, accepted only with the exact resource bytes they describe.
use kition_photo_doc::Document;
use kition_photo_psd::resources::ids;
use kition_photo_psd::{PsdFile, TaggedBlock};
use serde_json::{Value, json};

pub(crate) const BLOCK: &[u8; 4] = b"kRId";

/// The file the block travels in, which decides what its fingerprint covers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Container {
    /// A PSD or PSB: every resource, the channel count and the merged image data.
    Psd,
    /// A layered TIFF (`crate::tiff_layers`): the PSD model its importer rebuilds takes the
    /// merged image from the TIFF's pixels (without the alpha channels a TIFF does not keep) and
    /// the profile, resolution, XMP and EXIF resources from the TIFF's own tags, so those are
    /// left out. The keyed items (guides, paths, notes) live in the other resources and `Anno`.
    Tiff,
}

/// Resources a layered TIFF rebuilds from its own tags.
const TIFF_TAG_RESOURCES: [u16; 4] = [ids::ICC_PROFILE, ids::XMP, ids::RESOLUTION_INFO, ids::EXIF];

fn fingerprint(file: &PsdFile, container: Container) -> String {
    let psd = container == Container::Psd;
    let mut hash = blake3::Hasher::new();
    for n in [file.header.width, file.header.height, u32::from(file.header.depth)] {
        hash.update(&n.to_le_bytes());
    }
    if psd {
        hash.update(&u32::from(file.header.channels).to_le_bytes());
    }
    hash.update(&file.header.color_mode.as_u16().to_le_bytes());
    if psd {
        hash.update(&file.image_data.compression.as_u16().to_le_bytes());
    }
    for resource in file.resources.iter().filter(|r| psd || !TIFF_TAG_RESOURCES.contains(&r.id)) {
        hash.update(&resource.id.to_le_bytes());
        for bytes in [&resource.name, &resource.data] {
            hash.update(&(bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
    }
    for block in file.global_blocks.iter().filter(|b| &b.key == b"Anno") {
        hash.update(&(block.data.len() as u64).to_le_bytes());
        hash.update(&block.data);
    }
    if psd {
        hash.update(&file.image_data.data);
    }
    hash.finalize().to_hex().to_string()
}

/// Writes the block for `doc` into `file`, replacing one already there.
pub(crate) fn write(file: &mut PsdFile, doc: &Document, container: Container) {
    file.global_blocks.retain(|b| &b.key != BLOCK);
    let value = json!({"fingerprint":fingerprint(file, container),"channels":doc.channels.iter().map(|v|&v.key).collect::<Vec<_>>(),
        "paths":doc.paths.iter().map(|v|&v.key).collect::<Vec<_>>(),"notes":doc.notes.iter().map(|v|&v.key).collect::<Vec<_>>(),
        "horizontal":doc.guides.horizontal.iter().map(|v|&v.key).collect::<Vec<_>>(),"vertical":doc.guides.vertical.iter().map(|v|&v.key).collect::<Vec<_>>()});
    if let Ok(data) = serde_json::to_vec(&value) {
        let mut block = TaggedBlock::new(*BLOCK, data);
        // Document-level blocks are padded to a multiple of 4, like the export's other global
        // blocks (readers step to the next block that way, #200).
        block.padding = Some(vec![0; (4 - block.data.len() % 4) % 4]);
        file.global_blocks.push(block);
    }
}

pub(crate) fn read(file: &PsdFile, doc: &mut Document, container: Container) {
    // Generic PSD writers need not maintain this block. Changed bytes invalidate its claims.
    for channel in &mut doc.channels {
        channel.key = None;
    }
    let Some(block) = file.global_blocks.iter().find(|b| &b.key == BLOCK).filter(|b| b.data.len() <= 1024 * 1024) else { return };
    let Ok(value) = serde_json::from_slice::<Value>(&block.data) else { return };
    if value.get("fingerprint").and_then(Value::as_str) != Some(fingerprint(file, container).as_str()) {
        return;
    }
    let keys = |field: &str, len: usize| -> Option<Vec<Option<String>>> {
        let values = value.get(field)?.as_array()?;
        if values.len() != len {
            return None;
        }
        values.iter().map(|v| if v.is_null() { Some(None) } else { v.as_str().map(|s| Some(s.to_owned())) }).collect()
    };
    if let Some(keys) = keys("channels", doc.channels.len()) {
        for (v, key) in doc.channels.iter_mut().zip(keys) {
            v.key = key;
        }
    }
    if let Some(keys) = keys("paths", doc.paths.len()) {
        for (v, key) in doc.paths.iter_mut().zip(keys) {
            v.key = key;
        }
    }
    if let Some(keys) = keys("notes", doc.notes.len()) {
        for (v, key) in doc.notes.iter_mut().zip(keys) {
            v.key = key;
        }
    }
    if let Some(keys) = keys("horizontal", doc.guides.horizontal.len()) {
        for (v, key) in doc.guides.horizontal.iter_mut().zip(keys) {
            v.key = key;
        }
    }
    if let Some(keys) = keys("vertical", doc.guides.vertical.len()) {
        for (v, key) in doc.guides.vertical.iter_mut().zip(keys) {
            v.key = key;
        }
    }
}
