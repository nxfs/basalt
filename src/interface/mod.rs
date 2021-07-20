pub mod bin;
pub mod checkbox;
pub mod hook;
pub mod interface;
mod odb;
pub mod on_off_button;
pub mod render;
pub mod scroll_bar;
pub mod slider;

use fontdue::layout::{HorizontalAlign, VerticalAlign};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TextWrap {
	Shift,
	NewLine,
	None,
	NoneDotted,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FontWeight {
	Thin,
	ExtraLight,
	Light,
	Normal,
	Medium,
	SemiBold,
	Bold,
	ExtraBold,
	UltraBold,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TextHoriAlign {
	Left,
	Center,
	Right,
}

impl From<HorizontalAlign> for TextHoriAlign {
	fn from(from: HorizontalAlign) -> Self {
		match from {
			HorizontalAlign::Left => Self::Left,
			HorizontalAlign::Center => Self::Center,
			HorizontalAlign::Right => Self::Right,
		}
	}
}

impl Into<HorizontalAlign> for TextHoriAlign {
	fn into(self) -> HorizontalAlign {
		match self {
			Self::Left => HorizontalAlign::Left,
			Self::Center => HorizontalAlign::Center,
			Self::Right => HorizontalAlign::Right,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TextVertAlign {
	Top,
	Center,
	Bottom,
}

impl From<VerticalAlign> for TextVertAlign {
	fn from(from: VerticalAlign) -> Self {
		match from {
			VerticalAlign::Top => Self::Top,
			VerticalAlign::Middle => Self::Center,
			VerticalAlign::Bottom => Self::Bottom,
		}
	}
}

impl Into<VerticalAlign> for TextVertAlign {
	fn into(self) -> VerticalAlign {
		match self {
			Self::Top => VerticalAlign::Top,
			Self::Center => VerticalAlign::Middle,
			Self::Bottom => VerticalAlign::Bottom,
		}
	}
}
