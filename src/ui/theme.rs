use gpui::{App, Hsla, Rgba, Window, px, rgb, rgba};
use gpui_component::{Theme as ComponentTheme, ThemeMode as ComponentThemeMode};

pub(super) const MONO: &str = "JetBrains Mono";
pub(super) const FONT_UI: &str = ".SystemUIFont";
pub(super) const DESCRIPTION_MAX_WIDTH: f32 = 700.;
pub(super) const DESCRIPTION_COLLAPSED_LINES: usize = 3;
pub(super) const SETTINGS_CONTENT_MAX_WIDTH: f32 = 768.;
pub(super) const CONTEXT_TOKEN_WARNING_THRESHOLD: usize = 2_000;
pub(super) const CONTEXT_TOKEN_DANGER_THRESHOLD: usize = 5_000;
pub(super) const CONTEXT_COUNT_WARNING_THRESHOLD: usize = 20;
pub(super) const CONTEXT_COUNT_DANGER_THRESHOLD: usize = 50;

// Design tokens. Components read sizes from here instead of hard-coding them;
// see ui-review/design-system.html for the visual reference.

/// Toolbars, inline actions, menu items and tabs.
pub(super) const H_SM: f32 = 28.;
/// Form controls, dialog buttons and sidebar navigation.
pub(super) const H_MD: f32 = 32.;
/// Single-line list rows.
pub(super) const ROW_1: f32 = 32.;
/// Two-line list rows and card rows.
pub(super) const ROW_2: f32 = 48.;
/// Minimum height of a settings card row.
pub(super) const ROW_SETTING: f32 = 56.;
/// Height of a pane header (title + toolbar).
pub(super) const PANE_HEADER: f32 = 52.;
/// Agent / identity avatar diameter.
pub(super) const AVATAR: f32 = 24.;

// Radius scale. Nested corners stay concentric: inner = outer - padding.
pub(super) const R_SM: f32 = 6.; // badge, chip, menu item, tooltip, small icon tile
pub(super) const R_MD: f32 = 8.; // button, input, select, tab, list row
pub(super) const R_LG: f32 = 10.; // menu, popover, toast, 40px icon tile
pub(super) const R_XL: f32 = 12.; // card
/// Corner of a filled row flush against a card's 1px border, so its fill
/// follows the card's curve instead of poking out at the corners.
pub(super) const R_CARD_INNER: f32 = R_XL - 1.;
pub(super) const R_2XL: f32 = 16.; // modal

// Type scale.
pub(super) const FS_CAPTION: f32 = 11.;
pub(super) const FS_SMALL: f32 = 12.;
pub(super) const FS_BODY: f32 = 13.;
pub(super) const FS_EMPH: f32 = 14.;
pub(super) const FS_TITLE: f32 = 16.;
pub(super) const FS_DISPLAY: f32 = 20.;

// Icon sizes.
pub(super) const ICON_MD: f32 = 16.;
pub(super) const ICON_SM: f32 = 14.;
pub(super) const ICON_XS: f32 = 12.;

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub window: Rgba,
    pub window_border: Rgba,
    pub base: Rgba,
    pub sidebar: Rgba,
    pub surface: Rgba,
    /// Opaque background for menus, popovers and dialogs.
    pub elevated: Rgba,
    /// Opaque fill for avatars and stacked tiles that overlap each other.
    pub avatar: Rgba,
    pub raised: Rgba,
    pub hover: Rgba,
    pub selected: Rgba,
    pub border: Rgba,
    pub border_strong: Rgba,
    pub text: Rgba,
    pub sidebar_text: Rgba,
    pub secondary: Rgba,
    pub muted: Rgba,
    pub accent: Rgba,
    pub primary: Rgba,
    pub on_primary: Rgba,
    pub success: Rgba,
    pub warning: Rgba,
    pub danger: Rgba,
    pub danger_soft: Rgba,
    pub overlay: Rgba,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            window: rgb(0x141414),
            window_border: rgba(0xb0b0b04d),
            base: rgb(0x181818),
            sidebar: rgb(0x181818),
            surface: rgb(0x1c1c1c),
            elevated: rgb(0x262626),
            avatar: rgb(0x2a2a2a),
            raised: rgba(0xffffff0f),
            hover: rgba(0xffffff0d),
            selected: rgba(0xffffff1a),
            border: rgba(0xffffff14),
            border_strong: rgba(0xffffff24),
            text: rgb(0xdfdfdf),
            sidebar_text: rgba(0xffffffd9),
            secondary: rgba(0xffffffb5),
            muted: rgba(0xffffff80),
            accent: rgb(0x339cff),
            primary: rgb(0xececec),
            on_primary: rgb(0x0d0d0d),
            success: rgb(0x3fb950),
            warning: rgb(0xe3b341),
            danger: rgb(0xff6762),
            danger_soft: rgba(0xff67621f),
            overlay: rgba(0x00000066),
        }
    }

    pub fn light() -> Self {
        Self {
            window: rgb(0xf5f5f5),
            window_border: rgba(0x64646459),
            base: rgb(0xffffff),
            sidebar: rgba(0xffffffb3),
            surface: rgb(0xffffff),
            elevated: rgb(0xffffff),
            avatar: rgb(0xf2f2f2),
            raised: rgba(0x0d0d0d0d),
            hover: rgba(0x0d0d0d0a),
            selected: rgba(0x0d0d0d14),
            border: rgba(0x0d0d0d14),
            border_strong: rgba(0x0d0d0d21),
            text: rgb(0x0d0d0d),
            sidebar_text: rgba(0x0d0d0dd9),
            secondary: rgba(0x0d0d0db0),
            muted: rgba(0x0d0d0d7d),
            accent: rgb(0x0169cc),
            primary: rgb(0x0d0d0d),
            on_primary: rgb(0xffffff),
            success: rgb(0x1a7f37),
            warning: rgb(0xc98700),
            danger: rgb(0xe02e2a),
            danger_soft: rgba(0xe02e2a17),
            overlay: rgba(0x00000024),
        }
    }

    pub fn for_mode(mode: ComponentThemeMode) -> Self {
        if mode.is_dark() {
            Self::dark()
        } else {
            Self::light()
        }
    }
}

/// Switch gpui-component's theme mode and align the library-drawn widgets
/// (context menus, checkboxes, tooltips, inputs) with Kitter's tokens.
pub(super) fn apply_component_theme(
    mode: ComponentThemeMode,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    ComponentTheme::change(mode, window, cx);
    let p = Palette::for_mode(mode);
    let theme = ComponentTheme::global_mut(cx);
    theme.radius = px(R_MD);
    theme.radius_lg = px(R_2XL);
    // A 1px ring-coloured border is enough; the outer halo reads as heavy.
    theme.focus_ring = false;
    let colors: [(&mut Hsla, Rgba); 14] = [
        (&mut theme.colors.background, p.base),
        (&mut theme.colors.foreground, p.text),
        (&mut theme.colors.border, p.border),
        (&mut theme.colors.input, p.border_strong),
        (&mut theme.colors.ring, p.accent),
        (&mut theme.colors.popover, p.elevated),
        (&mut theme.colors.popover_foreground, p.text),
        (&mut theme.colors.accent, p.hover),
        (&mut theme.colors.accent_foreground, p.text),
        (&mut theme.colors.muted_foreground, p.muted),
        (&mut theme.colors.primary, p.primary),
        (&mut theme.colors.primary_foreground, p.on_primary),
        (&mut theme.colors.danger, p.danger),
        (&mut theme.colors.list_active, p.selected),
    ];
    for (slot, color) in colors {
        *slot = color.into();
    }
    theme.tokens.popover = theme.colors.popover.into();
    theme.tokens.primary = theme.colors.primary.into();
    theme.tokens.accent = theme.colors.accent.into();
}
