//! A widget for displaying and mutating multi-line text, given as a `String`.

use {
    Align,
    Color,
    Colorable,
    FontSize,
    NodeIndex,
    Point,
    Positionable,
    Range,
    Rect,
    Scalar,
    Sizeable,
    Widget,
};
use event;
use input;
use std;
use text;
use utils;
use widget;
use widget::primitive::text::Wrap;


/// A widget for displaying and mutating multi-line text, given as a `String`.
///
/// By default the text is wrapped via the first whitespace before the line exceeds the
/// `TextEdit`'s width, however a user may change this using the `.wrap_by_character` method.
pub struct TextEdit<'a> {
    common: widget::CommonBuilder,
    text: &'a str,
    style: Style,
}

widget_style!{
    /// Unique graphical styling for the TextEdit.
    style Style {
        /// The color of the text (this includes cursor and selection color).
        - color: Color { theme.shape_color }
        /// The font size for the text.
        - font_size: FontSize { theme.font_size_medium }
        /// The horizontal alignment of the text.
        - x_align: Align { Align::Start }
        /// The vertical alignment of the text.
        - y_align: Align { Align::End }
        /// The vertical space between each line of text.
        - line_spacing: Scalar { 1.0 }
        /// The way in which text is wrapped at the end of a line.
        - line_wrap: Wrap { Wrap::Whitespace }
        /// Do not allow to enter text that would exceed the bounds of the `TextEdit`'s `Rect`.
        - restrict_to_height: bool { true }
        /// The font used for the `Text`.
        - font_id: Option<text::font::Id> { theme.font_id }
    }
}

/// The State of the TextEdit widget that will be cached within the Ui.
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    cursor: Cursor,
    /// Track whether some sort of dragging is currently occurring.
    drag: Option<Drag>,
    /// Information about each line of text.
    line_infos: Vec<text::line::Info>,
    selected_rectangle_indices: Vec<NodeIndex>,
    rectangle_idx: widget::IndexSlot,
    text_idx: widget::IndexSlot,
    cursor_idx: widget::IndexSlot,
    highlight_idx: widget::IndexSlot,
}

/// Track whether some sort of dragging is currently occurring.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Drag {
    /// The drag is currently selecting a range of text.
    Selecting,
    /// The drag is moving a selection of text.
    #[allow(dead_code)] // TODO: Implement this.
    MoveSelection,
}

/// The position of the `Cursor` over the text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cursor {
    /// The cursor is at the given character index.
    Idx(text::cursor::Index),
    /// The cursor is a selection between these two indices.
    Selection {
        /// The `start` is always the "anchor" point.
        start: text::cursor::Index,
        /// The `end` may be either greater or less than the `start`.
        end: text::cursor::Index,
    },
}


impl<'a> TextEdit<'a> {

    /// Construct a TextEdit widget.
    pub fn new(text: &'a str) -> Self {
        TextEdit {
            common: widget::CommonBuilder::new(),
            text: text,
            style: Style::new(),
        }
    }

    /// The `TextEdit` will wrap text via the whitespace that precedes the first width-exceeding
    /// character.
    ///
    /// This is the default setting.
    pub fn wrap_by_whitespace(self) -> Self {
        self.line_wrap(Wrap::Whitespace)
    }

    /// By default, the `TextEdit` will wrap text via the whitespace that precedes the first
    /// width-exceeding character.
    ///
    /// Calling this method causes the `TextEdit` to wrap text at the first exceeding character.
    pub fn wrap_by_character(self) -> Self {
        self.line_wrap(Wrap::Character)
    }

    /// Align the text to the left of its bounding **Rect**'s *x* axis range.
    pub fn align_text_left(self) -> Self {
        self.x_align_text(Align::Start)
    }

    /// Align the text to the middle of its bounding **Rect**'s *x* axis range.
    pub fn align_text_x_middle(self) -> Self {
        self.x_align_text(Align::Middle)
    }

    /// Align the text to the right of its bounding **Rect**'s *x* axis range.
    pub fn align_text_right(self) -> Self {
        self.x_align_text(Align::End)
    }

    /// Align the text to the left of its bounding **Rect**'s *y* axis range.
    pub fn align_text_bottom(self) -> Self {
        self.y_align_text(Align::Start)
    }

    /// Align the text to the middle of its bounding **Rect**'s *y* axis range.
    pub fn align_text_y_middle(self) -> Self {
        self.y_align_text(Align::Middle)
    }

    /// Align the text to the right of its bounding **Rect**'s *y* axis range.
    pub fn align_text_top(self) -> Self {
        self.y_align_text(Align::End)
    }

    /// Align the text to the middle of its bounding **Rect**.
    pub fn align_text_middle(self) -> Self {
        self.align_text_x_middle().align_text_y_middle()
    }

    builder_methods!{
        pub font_size { style.font_size = Some(FontSize) }
        pub x_align_text { style.x_align = Some(Align) }
        pub y_align_text { style.y_align = Some(Align) }
        pub line_wrap { style.line_wrap = Some(Wrap) }
        pub line_spacing { style.line_spacing = Some(Scalar) }
        pub restrict_to_height { style.restrict_to_height = Some(bool) }
    }

}

impl<'a> Widget for TextEdit<'a> {
    type State = State;
    type Style = Style;
    // TODO: We should create a more specific `Event` type that:
    // - Allows for mutating an existing `String` directly
    // - Enumerates possible mutations (i.e. InsertChar, RemoveCharRange, etc).
    // - Enumerates cursor movement and range selection.
    type Event = Option<String>;

    fn common(&self) -> &widget::CommonBuilder {
        &self.common
    }

    fn common_mut(&mut self) -> &mut widget::CommonBuilder {
        &mut self.common
    }

    fn init_state(&self) -> State {
        State {
            cursor: Cursor::Idx(text::cursor::Index { line: 0, char: 0 }),
            drag: None,
            line_infos: Vec::new(),
            selected_rectangle_indices: Vec::new(),
            rectangle_idx: widget::IndexSlot::new(),
            text_idx: widget::IndexSlot::new(),
            cursor_idx: widget::IndexSlot::new(),
            highlight_idx: widget::IndexSlot::new(),
        }
    }

    fn style(&self) -> Style {
        self.style.clone()
    }

    /// Update the state of the TextEdit.
    fn update(self, args: widget::UpdateArgs<Self>) -> Self::Event {
        let widget::UpdateArgs { idx, state, rect, style, mut ui, .. } = args;
        let TextEdit { text, .. } = self;
        let mut text = std::borrow::Cow::Borrowed(text);

        // Retrieve the `font_id`, as long as a valid `Font` for it still exists.
        //
        // If we've no font to use for text logic, bail out without updating.
        let font_id = match style.font_id(&ui.theme)
            .or(ui.fonts.ids().next())
            .and_then(|id| ui.fonts.get(id).map(|_| id))
        {
            Some(font_id) => font_id,
            None => return None,
        };

        let font_size = style.font_size(ui.theme());
        let line_wrap = style.line_wrap(ui.theme());
        let x_align = style.x_align(ui.theme());
        let y_align = style.y_align(ui.theme());
        let line_spacing = style.line_spacing(ui.theme());
        let restrict_to_height = style.restrict_to_height(ui.theme());
        let text_idx = state.text_idx.get(&mut ui);

        /// Returns an iterator yielding the `text::line::Info` for each line in the given text
        /// with the given styling.
        type LineInfos<'a> = text::line::Infos<'a, text::line::NextBreakFnPtr>;
        fn line_infos<'a>(text: &'a str,
                          font: &'a text::Font,
                          font_size: FontSize,
                          line_wrap: Wrap,
                          max_width: Scalar) -> LineInfos<'a>
        {
            let infos = text::line::infos(text, font, font_size);
            match line_wrap {
                Wrap::Whitespace => infos.wrap_by_whitespace(max_width),
                Wrap::Character => infos.wrap_by_character(max_width),
            }
        }

        // Check to see if the given text has changed since the last time the widget was updated.
        {
            let maybe_new_line_infos = {
                let line_info_slice = &state.line_infos[..];
                let font = ui.fonts.get(font_id).unwrap();
                let new_line_infos = line_infos(&text, font, font_size, line_wrap, rect.w());
                match utils::write_if_different(line_info_slice, new_line_infos) {
                    std::borrow::Cow::Owned(new) => Some(new),
                    _ => None,
                }
            };

            if let Some(new_line_infos) = maybe_new_line_infos {
                state.update(|state| state.line_infos = new_line_infos);
            }
        }

        let xy_at = |cursor_idx: text::cursor::Index,
                     text: &str,
                     line_infos: &[text::line::Info],
                     font: &text::Font|
            -> Option<(Scalar, Range)>
        {
            let xys_per_line = text::cursor::xys_per_line_from_text(text,line_infos,font,font_size,
                                                                    x_align,y_align,line_spacing,rect);
            text::cursor::xy_at(xys_per_line, cursor_idx)
        };

        // Find the closest cursor index to the given `xy` position.
        //
        // Returns `None` if the given `text` is empty.
        let closest_cursor_index_and_xy = |xy: Point,
                                           text: &str,
                                           line_infos: &[text::line::Info],
                                           font: &text::Font|
            -> Option<(text::cursor::Index, Point)>
        {
            let xys_per_line = text::cursor::xys_per_line_from_text(text,line_infos,font,font_size,
                                                                    x_align,y_align,line_spacing,rect);
            text::cursor::closest_cursor_index_and_xy(xy,xys_per_line)
        };

        let get_index_on_line = |x_pos: Scalar,line_idx: usize,text: &str,
                                 line_infos: &[text::line::Info],
                                 font: &text::Font| -> Option<text::cursor::Index> {
            let mut xys_per_line = text::cursor::xys_per_line_from_text(text, line_infos, font,
                                                                        font_size, x_align, y_align,
                                                                        line_spacing, rect);
            xys_per_line.nth(line_idx).and_then(|(line_xs,_)| {
                let (char_idx,_) = text::cursor::closest_cursor_index_on_line(x_pos,line_xs);
                Some(text::cursor::Index { line: line_idx, char: char_idx })
            })
        };

        let mut cursor = state.cursor;
        let mut drag = state.drag;

        let insert_text = |string: &str, cursor: Cursor, text: &str, infos: &[text::line::Info], font: &text::Font|
            -> Option<(String,Cursor,std::vec::Vec<text::line::Info>)>
        {
            let string_char_count = string.chars().count();
            // Construct the new text with the new string inserted at the cursor.
            let (new_text, new_cursor_char_idx): (String, usize) = {
                let (cursor_start, cursor_end) = match cursor {
                    Cursor::Idx(idx) => (idx, idx),
                    Cursor::Selection { start, end } =>
                        (std::cmp::min(start, end), std::cmp::max(start, end)),
                };

                let line_infos = infos.iter().cloned();

                let (start_idx, end_idx) =
                    (text::glyph::index_after_cursor(line_infos.clone(), cursor_start)
                        .unwrap_or(0),
                     text::glyph::index_after_cursor(line_infos.clone(), cursor_end)
                        .unwrap_or(0));

                let new_cursor_char_idx = start_idx + string_char_count;

                let new_text = text.chars().take(start_idx)
                    .chain(string.chars())
                    .chain(text.chars().skip(end_idx))
                    .collect();
                (new_text, new_cursor_char_idx)
            };

            // Calculate the new `line_infos` for the `new_text`.
            let new_line_infos: Vec<_> = {
                line_infos(&new_text, font, font_size, line_wrap, rect.w()).collect()
            };

            // Check that the new text would not exceed the `inner_rect` bounds.
            let num_lines = new_line_infos.len();
            let height = text::height(num_lines, font_size, line_spacing);
            if height < rect.h() || !restrict_to_height {
                // Determine the new `Cursor` and its position.
                let new_cursor_idx = {
                    let line_infos = new_line_infos.iter().cloned();
                    text::cursor::index_before_char(line_infos, new_cursor_char_idx)
                        .unwrap_or(text::cursor::Index {
                            line: 0,
                            char: string_char_count,
                        })
                };
                Some((new_text, Cursor::Idx(new_cursor_idx), new_line_infos))
            } else {
                None
            }
        };

        // Check for the following events:
        // - `Text` events for receiving new text.
        // - Left mouse `Press` events for either:
        //     - setting the cursor or start of a selection.
        //     - begin dragging selected text.
        // - Left mouse `Drag` for extending the end of the selection, or for dragging selected text.
        'events: for widget_event in ui.widget_input(idx).events() {
            match widget_event {

                event::Widget::Press(press) => match press.button {

                    // If the left mouse button was pressed, place a `Cursor` with the starting
                    // index at the mouse position.
                    event::Button::Mouse(input::MouseButton::Left, rel_xy) => {
                        let abs_xy = utils::vec2_add(rel_xy, rect.xy());
                        let infos = &state.line_infos;
                        let font = ui.fonts.get(font_id).unwrap();
                        let closest = closest_cursor_index_and_xy(abs_xy, &text, infos, font);
                        if let Some((closest_cursor, _)) = closest {
                            cursor = Cursor::Idx(closest_cursor);
                        }

                        // TODO: Differentiate between Selecting and MoveSelection.
                        drag = Some(Drag::Selecting);
                    }

                    // Check for control keys.
                    event::Button::Keyboard(key) => match key {

                        // If `Cursor::Idx`, remove the `char` behind the cursor.
                        // If `Cursor::Selection`, remove the selected text.
                        input::Key::Backspace => {
                            match cursor {

                                Cursor::Idx(cursor_idx) => {
                                    let idx_after_cursor = {
                                        let line_infos = state.line_infos.iter().cloned();
                                        text::glyph::index_after_cursor(line_infos, cursor_idx)
                                    };
                                    if let Some(idx) = idx_after_cursor {
                                        if idx > 0 {
                                            let idx_to_remove = idx - 1;

                                            *text.to_mut() = text.chars().take(idx_to_remove)
                                                .chain(text.chars().skip(idx))
                                                .collect();

                                            state.update(|state| {
                                                let font = ui.fonts.get(font_id).unwrap();
                                                let w = rect.w();
                                                state.line_infos =
                                                    line_infos(&text, font, font_size, line_wrap, w)
                                                        .collect();
                                            });

                                            let line_infos = state.line_infos.iter().cloned();
                                            let new_cursor_idx =
                                                 text::cursor::index_before_char(line_infos, idx_to_remove)
                                                 // in case we removed the last character
                                                .unwrap_or(text::cursor::Index {line: 0, char: 0});
                                            cursor = Cursor::Idx(new_cursor_idx);
                                        }
                                    }
                                },

                                Cursor::Selection { start, end } => {
                                    let (start_idx, end_idx) = {
                                        let line_infos = state.line_infos.iter().cloned();
                                        (text::glyph::index_after_cursor(line_infos.clone(), start)
                                            .expect("text::cursor::Index was out of range"),
                                         text::glyph::index_after_cursor(line_infos, end)
                                            .expect("text::cursor::Index was out of range"))
                                    };
                                    let (start_idx, end_idx) =
                                        if start_idx <= end_idx { (start_idx, end_idx) }
                                        else                    { (end_idx, start_idx) };
                                    let new_cursor_char_idx =
                                        if start_idx > 0 { start_idx } else { 0 };
                                    let new_cursor_idx = {
                                        let line_infos = state.line_infos.iter().cloned();
                                        text::cursor::index_before_char(line_infos, new_cursor_char_idx)
                                            .expect("char index was out of range")
                                    };
                                    cursor = Cursor::Idx(new_cursor_idx);
                                    *text.to_mut() = text.chars().take(start_idx)
                                        .chain(text.chars().skip(end_idx))
                                        .collect();
                                    state.update(|state| {
                                        let font = ui.fonts.get(font_id).unwrap();
                                        let w = rect.w();
                                        state.line_infos =
                                            line_infos(&text, font, font_size, line_wrap, w)
                                                .collect();
                                    });
                                },

                            }
                        },

                        input::Key::Left => {
                            if !press.modifiers.contains(input::keyboard::CTRL) {
                                match cursor {

                                    // Move the cursor to the previous position.
                                    Cursor::Idx(cursor_idx) => {
                                        let new_cursor_idx = {
                                            let line_infos = state.line_infos.iter().cloned();
                                            cursor_idx.previous(line_infos).unwrap_or(cursor_idx)
                                        };
                                        cursor = Cursor::Idx(new_cursor_idx);
                                    },

                                    // Move the cursor to the start of the current selection.
                                    Cursor::Selection { start, end } => {
                                        let new_cursor_idx = std::cmp::min(start, end);
                                        cursor = Cursor::Idx(new_cursor_idx);
                                    },
                                }
                            }
                        },

                        input::Key::Right => {
                            if !press.modifiers.contains(input::keyboard::CTRL) {
                                match cursor {

                                    // Move the cursor to the next position.
                                    Cursor::Idx(cursor_idx) => {
                                        let new_cursor_idx = {
                                            let line_infos = state.line_infos.iter().cloned();
                                            cursor_idx.next(line_infos).unwrap_or(cursor_idx)
                                        };

                                        cursor = Cursor::Idx(new_cursor_idx);
                                    },

                                    // Move the cursor to the end of the current selection.
                                    Cursor::Selection { start, end } => {
                                        let new_cursor_idx = std::cmp::max(start, end);
                                        cursor = Cursor::Idx(new_cursor_idx);
                                    },
                                }
                            }
                        },

                        input::Key::Up | input::Key::Down => {
                            let cursor_idx = match cursor {
                                Cursor::Idx(cursor_idx) => cursor_idx,
                                Cursor::Selection { start, .. } => start,
                            };
                            let font = ui.fonts.get(font_id).unwrap();
                            let new_cursor_idx = xy_at(cursor_idx, &text, &state.line_infos, font).and_then(|(x_pos,_)| {
                                let text::cursor::Index { line, .. } = cursor_idx;
                                let next_line = match key {
                                    input::Key::Up => if line > 0 { line - 1 } else { 0 },
                                    input::Key::Down => line + 1,
                                    _ => unreachable!()
                                };
                                get_index_on_line(x_pos, next_line, &text, &state.line_infos, font)
                            }).unwrap_or(cursor_idx);
                            cursor = Cursor::Idx(new_cursor_idx);
                        },

                        input::Key::A => {
                            // Select all text on Ctrl+a.
                            if press.modifiers.contains(input::keyboard::CTRL) {
                                let start = text::cursor::Index { line: 0, char: 0 };
                                let end = {
                                    let line_infos = state.line_infos.iter().cloned();
                                    text::cursor::index_before_char(line_infos, text.chars().count())
                                        .expect("char index was out of range")
                                };
                                cursor = Cursor::Selection { start: start, end: end };
                            }
                        },

                        input::Key::E => {
                            // If cursor is `Idx`, move cursor to end.
                            if press.modifiers.contains(input::keyboard::CTRL) {
                            }
                        },

                        input::Key::Return => {
                            match insert_text("\n", cursor, &text, &state.line_infos, ui.fonts.get(font_id).unwrap()) {
                                Some((new_text, new_cursor, new_line_infos)) => {
                                    *text.to_mut() = new_text;
                                    cursor = new_cursor;
                                    state.update(|state| state.line_infos = new_line_infos);
                                }, _ => ()
                            }
                        },

                        _ => (),
                    },

                    _ => (),

                },

                event::Widget::Release(release) => {
                    // Release drag.
                    if let event::Button::Mouse(input::MouseButton::Left, _) = release.button {
                        drag = None;
                    }
                },

                event::Widget::Text(event::Text { string, modifiers }) => {
                    if modifiers.contains(input::keyboard::CTRL)
                    || string.chars().count() == 0
                    || string.chars().next().is_none() {
                        continue 'events;
                    }

                    // Ignore text produced by arrow keys.
                    // 
                    // TODO: These just happened to be the modifiers for the arrows on OS X, I've
                    // no idea if they also apply to other platforms. We should definitely see if
                    // there's a better way to handle this, or whether this should be fixed
                    // upstream.
                    match &string[..] {
                        "\u{f700}" | "\u{f701}" | "\u{f702}" | "\u{f703}" => continue 'events,
                        _ => ()
                    }
                    match insert_text(&string, cursor, &text, &state.line_infos, ui.fonts.get(font_id).unwrap()) {
                        Some((new_text, new_cursor, new_line_infos)) => {
                            *text.to_mut() = new_text;
                            cursor = new_cursor;
                            state.update(|state| state.line_infos = new_line_infos);
                        }, _ => ()
                    }
                },

                // Check whether or not 
                event::Widget::Drag(drag_event) => {
                    if let input::MouseButton::Left = drag_event.button {
                        match drag {

                            Some(Drag::Selecting) => {
                                let start_cursor_idx = match cursor {
                                    Cursor::Idx(idx) => idx,
                                    Cursor::Selection { start, .. } => start,
                                };
                                let abs_xy = utils::vec2_add(drag_event.to, rect.xy());
                                let infos = &state.line_infos;
                                let font = ui.fonts.get(font_id).unwrap();
                                match closest_cursor_index_and_xy(abs_xy, &text, infos, font) {
                                    Some((end_cursor_idx, _)) =>
                                        cursor = Cursor::Selection {
                                            start: start_cursor_idx,
                                            end: end_cursor_idx,
                                        },
                                    _ => (),
                                }
                            },

                            // TODO: This should move the selected text.
                            Some(Drag::MoveSelection) => {
                                unimplemented!();
                            },

                            None => (),
                        }
                    }
                },

                _ => (),
            }
        }

        if state.cursor != cursor {
            state.update(|state| state.cursor = cursor);
        }

        if state.drag != drag {
            state.update(|state| state.drag = drag);
        }

        /// Takes the `String` from the `Cow` if the `Cow` is `Owned`.
        fn take_if_owned(text: std::borrow::Cow<str>) -> Option<String> {
            match text {
                std::borrow::Cow::Borrowed(_) => None,
                std::borrow::Cow::Owned(s) => Some(s),
            }
        }

        let color = style.color(ui.theme());
        let font_size = style.font_size(ui.theme());
        let num_lines = state.line_infos.iter().count();
        let text_height = text::height(num_lines, font_size, line_spacing);
        let text_y_range = Range::new(0.0, text_height).align_to(y_align, rect.y);
        let text_rect = Rect { x: rect.x, y: text_y_range };

        match line_wrap {
            Wrap::Whitespace => widget::Text::new(&text).wrap_by_word(),
            Wrap::Character => widget::Text::new(&text).wrap_by_character(),
        }
            .wh(text_rect.dim())
            .xy(text_rect.xy())
            .align_text_to(x_align)
            .graphics_for(idx)
            .color(color)
            .line_spacing(line_spacing)
            .font_size(font_size)
            .set(text_idx, &mut ui);

        // Draw the line for the cursor.
        let cursor_idx = match cursor {
            Cursor::Idx(idx) => idx,
            Cursor::Selection { end, .. } => end,
        };

        // If this widget is not capturing the keyboard, no need to draw cursor or selection.
        if ui.global_input().current.widget_capturing_keyboard != Some(idx) {
            return take_if_owned(text);
        }

        let (cursor_x, cursor_y_range) = {
            let font = ui.fonts.get(font_id).unwrap();
            xy_at(cursor_idx, &text, &state.line_infos, font)
                .unwrap_or_else(|| {
                    let x = rect.left();
                    let y = Range::new(0.0, font_size as Scalar).align_to(y_align, rect.y);
                    (x, y)
                })
        };

        let cursor_line_idx = state.cursor_idx.get(&mut ui);
        let start = [0.0, cursor_y_range.start];
        let end = [0.0, cursor_y_range.end];
        widget::Line::centred(start, end)
            .x_y(cursor_x, cursor_y_range.middle())
            .graphics_for(idx)
            .parent(idx)
            .color(color)
            .set(cursor_line_idx, &mut ui);

        if let Cursor::Selection { start, end } = cursor {
            let (start, end) = (std::cmp::min(start, end), std::cmp::max(start, end));

            let selected_rects: Vec<Rect> = {
                let line_infos = state.line_infos.iter().cloned();
                let lines = line_infos.clone().map(|info| &text[info.byte_range()]);
                let line_rects = text::line::rects(line_infos.clone(), font_size, rect,
                                                   x_align, y_align, line_spacing);
                let lines_with_rects = lines.zip(line_rects.clone());
                let font = ui.fonts.get(font_id).unwrap();
                text::line::selected_rects(lines_with_rects, font, font_size, start, end).collect()
            };

            // Draw a semi-transparent `Rectangle` for the selected range across each line.
            let selected_rect_color = color.highlighted().alpha(0.25);
            for (i, selected_rect) in selected_rects.iter().enumerate() {
                if i == state.selected_rectangle_indices.len() {
                    state.update(|state| {
                        state.selected_rectangle_indices.push(ui.new_unique_node_index());
                    });
                }
                let selected_rectangle_idx = state.selected_rectangle_indices[i];

                widget::Rectangle::fill(selected_rect.dim())
                    .xy(selected_rect.xy())
                    .color(selected_rect_color)
                    .graphics_for(idx)
                    .parent(idx)
                    .set(selected_rectangle_idx, &mut ui);
            }
        }

        take_if_owned(text)
    }

}


impl<'a> Colorable for TextEdit<'a> {
    builder_method!(color { style.color = Some(Color) });
}