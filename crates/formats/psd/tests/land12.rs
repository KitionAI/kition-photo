//! An empty layer table cannot signal merged transparency through a negative count.
use kition_photo_psd::{Compression, TaggedBlock, Version, layer::LayerInfo, testgen};

#[test]
fn empty_layer_table_reads_the_merged_transparency_marker() {
    for key in [*b"Mtrn", *b"Mt16", *b"Mt32"] {
        let mut file = testgen::small(Version::Psd, Compression::Raw);
        file.header.channels = 4;
        file.layer_info = Some(LayerInfo::default());
        assert!(!file.merged_has_alpha());
        file.global_blocks.push(TaggedBlock::new(key, Vec::new()));
        assert!(file.merged_has_alpha(), "{key:?}");
    }
}
