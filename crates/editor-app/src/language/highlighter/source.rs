//! Zero-copy byte access to immutable Base Rope snapshots, including reads inside UTF-8 characters.
use gpui_base::input::Rope;

/// Tree-sitter requests bytes rather than character boundaries; slicing bytes avoids FFI panics.
pub(super) fn read(source: &Rope, offset: usize) -> &[u8] {
    if offset >= source.len() {
        return &[];
    }
    let (chunk, start) = source.chunk(offset);
    &chunk.as_bytes()[offset - start..]
}
/// Query predicates borrow only the captured node's chunks, with no flattened document copy.
pub(super) struct Text<'a>(pub &'a Rope);
/// A bounded iterator keeps original source coordinates across chunk boundaries.
pub(super) struct Bytes<'a> {
    /// Shared immutable snapshot owned by EditorState's parsing caller.
    source: &'a Rope,
    /// Next byte requested by a Tree-sitter predicate.
    offset: usize,
    /// Exclusive captured-node boundary, clamped to the current snapshot.
    end: usize,
}
impl<'a> tree_sitter::TextProvider<&'a [u8]> for Text<'a> {
    type I = Bytes<'a>;
    fn text(&mut self, node: tree_sitter::Node) -> Self::I {
        Bytes {
            source: self.0,
            offset: node.start_byte().min(self.0.len()),
            end: node.end_byte().min(self.0.len()),
        }
    }
}
impl<'a> Iterator for Bytes<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.end {
            return None;
        }
        let bytes = read(self.source, self.offset);
        let length = bytes.len().min(self.end - self.offset);
        self.offset += length;
        (length > 0).then_some(&bytes[..length])
    }
}
