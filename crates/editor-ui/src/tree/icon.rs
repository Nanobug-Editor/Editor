//! Native tree artwork keeps literal SVG colors, while currentColor icons inherit row foreground.
use crate::Icon;
use gpui_kit::{
    AnyElement, Image, ImageFormat, Img, InteractiveElement as _, IntoElement, ParentElement as _,
    Styled as _, div, img, px,
};
use std::sync::Arc;

/// One fixed 16px slot accepts existing themed icons or bounded, runtime-admitted package artwork.
pub enum TreeRowIcon {
    /// Monochrome artwork follows the native row's selected or ordinary foreground.
    Tint(Icon),
    /// Literal-color artwork follows its supplied theme variant, as Explorer's file icons do.
    Color(Arc<Image>),
    /// Provider trees without explicit artwork share Explorer's live file/folder icon resolution.
    File(Img),
}

impl From<Icon> for TreeRowIcon {
    fn from(icon: Icon) -> Self {
        Self::Tint(icon)
    }
}

impl TreeRowIcon {
    /// Resolve already-admitted SVG bytes; currentColor retains the existing tint path, while
    /// small literal artwork uses GPUI's content-addressed image decoder instead of its alpha mask.
    /// Bound intrinsic dimensions before the decoder allocates; large artwork keeps the 16px mask.
    pub fn artwork(bytes: &[u8]) -> Self {
        let options = resvg::usvg::Options {
            image_href_resolver: resvg::usvg::ImageHrefResolver {
                resolve_data: Box::new(|_, _, _| None),
                resolve_string: Box::new(|_, _| None),
            },
            ..Default::default()
        };
        let small_literal = !bytes
            .windows(b"currentColor".len())
            .any(|part| part.eq_ignore_ascii_case(b"currentColor"))
            && std::str::from_utf8(bytes)
                .ok()
                .and_then(|source| resvg::usvg::Tree::from_str(source, &options).ok())
                .is_some_and(|tree| tree.size().width() <= 64. && tree.size().height() <= 64.);
        if small_literal {
            Self::Color(Arc::new(Image::from_bytes(
                ImageFormat::Svg,
                bytes.to_vec(),
            )))
        } else {
            Self::Tint(Icon::default().data(bytes))
        }
    }

    /// Keep disclosure/label geometry identical for both render paths; image IDs hash the bytes so
    /// unchanged folder artwork reuses GPUI's cache across rows and periodic tree publications.
    pub(crate) fn element(self) -> AnyElement {
        let (kind, child) = match self {
            Self::Tint(icon) => (
                "tree-row-icon-tint",
                icon.size(px(16.)).flex_shrink_0().into_any_element(),
            ),
            Self::Color(image) => (
                "tree-row-icon-color",
                img(image).size(px(16.)).flex_shrink_0().into_any_element(),
            ),
            Self::File(image) => ("tree-row-icon-file", image.into_any_element()),
        };
        div()
            .id("tree-row-icon")
            .debug_selector(move || kind.into())
            .size(px(16.))
            .flex_shrink_0()
            .child(child)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::SvgRenderer;
    use std::collections::BTreeSet;

    /// Literal viewport dimensions cannot turn a 16px tree icon into an unbounded image allocation.
    #[test]
    fn oversized_tree_artwork_keeps_the_bounded_mask_renderer() {
        let bytes = br##"<svg xmlns="http://www.w3.org/2000/svg" width="1000000" height="1000000"><path fill="#abcdef" d="M0 0h1v1z"/></svg>"##;
        assert!(matches!(TreeRowIcon::artwork(bytes), TreeRowIcon::Tint(_)));
    }

    /// The actual Explorer folder artwork must reach the color decoder with multiple RGB values.
    #[test]
    fn tree_folder_artwork_preserves_explorer_colors_in_both_themes() {
        for bytes in [
            include_bytes!("../../assets/file-icons/folder.svg").as_slice(),
            include_bytes!("../../assets/file-icons/folder_dark.svg").as_slice(),
        ] {
            let TreeRowIcon::Color(image) = TreeRowIcon::artwork(bytes) else {
                panic!("literal folder colors require image rendering")
            };
            let rendered = image.to_image_data(SvgRenderer::new(Arc::new(()))).unwrap();
            let colors = rendered
                .as_bytes(0)
                .unwrap()
                .chunks_exact(4)
                .filter(|pixel| pixel[3] == 255)
                .map(|pixel| [pixel[0], pixel[1], pixel[2]])
                .collect::<BTreeSet<_>>();
            assert!(
                colors.len() > 1,
                "folder fill and outline cannot collapse into one tint"
            );
            let TreeRowIcon::Color(again) = TreeRowIcon::artwork(bytes) else {
                unreachable!()
            };
            assert_eq!(
                image.id(),
                again.id(),
                "unchanged artwork reuses its decoded image"
            );
        }
    }
}
