//! Minimal terminal user interface (TUI) implementation.
//! It's inspired in the tiling window manager system, where the user always have the whole screen
//! covered and it just splits it between different widgets.
use std::io::{Read, Write, stdout};
use std::ops::{Add, AddAssign};
use std::{mem::MaybeUninit, os::fd::AsRawFd};

use libc::termios as Termios;

pub trait Widget: CommonWidget {
    fn render(&self, buffer: &mut Buffer);
}

pub trait CommonWidget {
    fn size(&self) -> Size;
    fn usable_size(&self) -> Size;

    fn set_vertical_alignment(&mut self, vertical_alignment: VerticalAlignment);
    fn set_horizontal_alignment(&mut self, horizontal_alignment: HorizontalAlignment);
    fn set_border(&mut self, color: Option<Color>);
    fn set_title(&mut self, title: Option<String>);

    fn rendering_region(self) -> RenderingRegion;
}

macro_rules! implement_common_widget {
    ($type:ty) => {
        impl $crate::tui::CommonWidget for $type {
            fn size(&self) -> $crate::tui::Size {
                self.rendering_region.size
            }

            fn set_border(&mut self, color: Option<$crate::tui::Color>) {
                self.rendering_region.set_border(color)
            }

            fn set_title(&mut self, title: Option<String>) {
                self.rendering_region.set_title(title);
            }

            fn set_vertical_alignment(
                &mut self,
                vertical_alignment: $crate::tui::VerticalAlignment,
            ) {
                self.rendering_region.vertical_alignment = vertical_alignment
            }

            fn set_horizontal_alignment(
                &mut self,
                horizontal_alignment: $crate::tui::HorizontalAlignment,
            ) {
                self.rendering_region.horizontal_alignment = horizontal_alignment
            }

            fn usable_size(&self) -> $crate::tui::Size {
                self.rendering_region.usable_size()
            }

            fn rendering_region(self) -> $crate::tui::RenderingRegion {
                self.rendering_region
            }
        }
    };
}

#[derive(Default, Clone, Copy)]
pub struct Vector2 {
    x: usize,
    y: usize,
}

impl Vector2 {
    fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

impl Add<Vector2> for Vector2 {
    type Output = Vector2;

    fn add(mut self, rhs: Vector2) -> Self::Output {
        self.x += rhs.x;
        self.y += rhs.y;

        self
    }
}

impl AddAssign for Vector2 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

pub struct Buffer {
    size: Size,
    data: Vec<Cell>,
}

impl Buffer {
    fn new(size: Size) -> Self {
        Buffer {
            size,
            data: vec![Cell::default(); size.width * size.height],
        }
    }

    #[inline(always)]
    fn cell_mut(&mut self, position: Vector2) -> &mut Cell {
        debug_assert!(position.x <= self.size.width);
        debug_assert!(position.y <= self.size.height);

        &mut self.data[self.size.width * position.y + position.x]
    }
}

pub struct Terminal {
    pub buffer: Buffer,
    tty: std::fs::File,
    termios: Termios,
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Err(err) = self.disable_raw_mode() {
            eprintln!(
                "ERROR: Could not return the terminal to canonical mode, run 'reset' to force it back: {err}"
            )
        };

        Terminal::leave_alternate_screen();
        Terminal::make_cursor_visible();
    }
}

impl Terminal {
    pub fn try_new() -> std::io::Result<Terminal> {
        let tty = std::fs::File::open("/dev/tty")?;

        let termios = Terminal::init_termios(&tty)?;
        let size = Terminal::size().unwrap();

        let terminal = Terminal {
            buffer: Buffer::new(size),
            tty,
            termios,
        };

        terminal.enable_raw_mode()?;

        Terminal::enter_alternate_screen();
        Terminal::make_cursor_invisible();

        Ok(terminal)
    }

    fn init_termios(tty: &std::fs::File) -> Result<Termios, std::io::Error> {
        unsafe {
            let mut termios: MaybeUninit<Termios> = MaybeUninit::uninit();

            if libc::tcgetattr(tty.as_raw_fd(), termios.as_mut_ptr()) < 0 {
                return Err(std::io::Error::last_os_error());
            }

            Ok(termios.assume_init())
        }
    }

    fn enable_raw_mode(&self) -> std::io::Result<()> {
        // We keep the original Termios untouched so we can reset it's state back
        let mut termios = self.termios;

        unsafe { libc::cfmakeraw(&mut termios) }

        unsafe {
            if libc::tcsetattr(self.tty.as_raw_fd(), libc::TCSANOW, &termios) < 0 {
                return Err(std::io::Error::last_os_error());
            }
        }

        Ok(())
    }

    fn disable_raw_mode(&mut self) -> std::io::Result<()> {
        unsafe {
            if libc::tcsetattr(self.tty.as_raw_fd(), libc::TCSANOW, &self.termios) < 0 {
                return Err(std::io::Error::last_os_error());
            };
        }

        Ok(())
    }

    pub fn draw(&mut self) {
        Terminal::move_cursor_to_home_position();

        // We always start with the Default color to ensure consistency
        let mut current_foreground_color = Color::Default;
        let mut current_background_color = Color::Default;
        current_foreground_color.apply_foreground();
        current_background_color.apply_background();

        for cell in &self.buffer.data {
            if cell.foreground_color != current_foreground_color {
                current_foreground_color = cell.foreground_color;
                current_foreground_color.apply_foreground();
            }

            if cell.background_color != current_background_color {
                current_background_color = cell.background_color;
                current_background_color.apply_background();
            }

            print!("{}", cell.character)
        }

        stdout().flush().unwrap();
        self.buffer.data.fill(Cell::default())
    }

    pub fn rendering_region(&self) -> RenderingRegion {
        let size = self.buffer.size;

        RenderingRegion::new(Vector2::default(), size)
    }

    fn size() -> std::io::Result<Size> {
        #[repr(C)]
        struct TermSize {
            row: libc::c_ushort,
            col: libc::c_ushort,
            x: libc::c_ushort,
            y: libc::c_ushort,
        }

        unsafe {
            let mut size: TermSize = std::mem::zeroed();
            if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) < 0 {
                return Err(std::io::Error::last_os_error());
            }

            Ok(Size {
                width: size.col as usize,
                height: size.row as usize,
            })
        }
    }

    fn enter_alternate_screen() {
        print!("\x1b[?1049h");
    }

    fn leave_alternate_screen() {
        print!("\x1b[?1049l");
    }

    fn move_cursor_to_home_position() {
        print!("\x1B[H");
    }

    fn make_cursor_invisible() {
        print!("\x1b[?25l");
    }

    fn make_cursor_visible() {
        print!("\x1b[?25h");
    }

    pub fn tty(&self) -> std::io::Result<std::io::Bytes<std::fs::File>> {
        self.tty.try_clone().map(|file| file.bytes())
    }
}

#[derive(Clone, Copy, Default)]
pub struct Size {
    pub width: usize,
    pub height: usize,
}

impl Size {
    fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }
}

#[derive(Default)]
pub struct RenderingRegion {
    title: Option<String>,
    position: Vector2,
    pub size: Size,
    border_color: Option<Color>,
    vertical_alignment: VerticalAlignment,
    horizontal_alignment: HorizontalAlignment,
}

impl RenderingRegion {
    fn new(position: Vector2, size: Size) -> RenderingRegion {
        RenderingRegion {
            position,
            size,
            ..Default::default()
        }
    }

    /// Vertical split
    /// +-----++-----+
    /// |     ||     |
    /// |     ||     |
    /// |     ||     |
    /// |     ||     |
    /// +-----++-----+
    pub fn split_vertically(self) -> (RenderingRegion, RenderingRegion) {
        self.split_vertically_at_percentage(0.5)
    }

    pub fn split_vertically_at_percentage(
        self,
        percentage: f32,
    ) -> (RenderingRegion, RenderingRegion) {
        assert!(percentage > 0.0 && percentage < 1.0);
        let offset = self.size.width as f32 * percentage;
        self.split_vertically_at(offset as u32)
    }

    pub fn split_vertically_at(self, offset: u32) -> (RenderingRegion, RenderingRegion) {
        assert!(offset <= self.size.width as u32);

        let left_width = offset as usize;
        let right_width = self.size.width - left_width;

        let left = RenderingRegion {
            position: self.position,
            size: Size::new(left_width, self.size.height),
            ..Default::default()
        };
        let right = RenderingRegion {
            position: self.position + Vector2::new(left_width, 0),
            size: Size::new(right_width, self.size.height),
            ..Default::default()
        };

        (left, right)
    }

    /// Horizontal split
    /// +------------+
    /// |            |
    /// +------------+
    /// +------------+
    /// |            |
    /// +------------+
    pub fn split_horizontally(self) -> (RenderingRegion, RenderingRegion) {
        self.split_horizontally_percentage(0.5)
    }

    pub fn split_horizontally_percentage(
        self,
        percentage: f32,
    ) -> (RenderingRegion, RenderingRegion) {
        assert!(percentage > 0.0 && percentage < 1.0);
        let offset = self.size.height as f32 * percentage;
        self.split_horizontally_at(offset as u32)
    }

    pub fn split_horizontally_at(self, offset: u32) -> (RenderingRegion, RenderingRegion) {
        assert!(offset <= self.size.height as u32);

        let top_height = offset as usize;
        let bottom_height = self.size.height - top_height;

        let top = RenderingRegion {
            position: self.position,
            size: Size::new(self.size.width, top_height),
            ..Default::default()
        };

        let bottom = RenderingRegion {
            position: self.position + Vector2::new(0, top_height),
            size: Size::new(self.size.width, bottom_height),
            ..Default::default()
        };

        (top, bottom)
    }

    pub fn text(self) -> Text {
        Text::new(self)
    }

    pub fn item_list(self) -> ItemList {
        ItemList::new(self)
    }

    pub fn table(self) -> Table {
        Table::new(self)
    }

    #[inline(always)]
    fn vertical_offset(&self, content_length: usize) -> usize {
        let border_offset = self.border_offset();
        let content_length = usize::min(content_length, self.usable_size().height);

        match self.vertical_alignment {
            VerticalAlignment::Top => border_offset,
            VerticalAlignment::Bottom => self.size.height - border_offset - content_length,
            VerticalAlignment::Center => (self.size.height - content_length) / 2,
        }
    }

    #[inline(always)]
    fn horizontal_offset(&self, content_length: usize) -> usize {
        let border_offset = self.border_offset();
        let content_length = usize::min(content_length, self.usable_size().width);

        match self.horizontal_alignment {
            HorizontalAlignment::Left => border_offset,
            HorizontalAlignment::Right => self.size.width - border_offset - content_length,
            HorizontalAlignment::Center => (self.size.width - content_length) / 2,
        }
    }

    #[inline(always)]
    pub fn usable_size(&self) -> Size {
        let border_offset = self.border_offset();
        Size {
            width: self.size.width - 2 * border_offset,
            height: self.size.height - 2 * border_offset,
        }
    }

    #[inline(always)]
    fn border_offset(&self) -> usize {
        if self.border_color.is_some() { 1 } else { 0 }
    }

    #[inline(always)]
    fn cell_mut<'a>(&self, buffer: &'a mut Buffer, position: Vector2) -> &'a mut Cell {
        debug_assert!(position.x <= self.size.width);
        debug_assert!(position.y <= self.size.height);

        buffer.cell_mut(self.position + position)
    }

    fn highlight_row(&self, buffer: &mut Buffer, selected_row: usize) {
        for column in 0..self.size.width {
            let cell = self.cell_mut(buffer, Vector2::new(column, selected_row));

            cell.background_color = Color::Cyan;
            cell.foreground_color = Color::Black;
        }
    }

    fn render(&self, buffer: &mut Buffer) {
        if let Some(border_color) = self.border_color {
            for y in 0..self.size.height {
                for x in 0..self.size.width {
                    let cell = self.cell_mut(buffer, Vector2::new(x, y));

                    if y == 0 {
                        if x == 0 {
                            cell.character = '┌';
                            cell.foreground_color = border_color;
                        } else if x == self.size.width - 1 {
                            cell.character = '┐';
                            cell.foreground_color = border_color;
                        } else {
                            cell.character = '─';
                            cell.foreground_color = border_color;
                        }
                    } else if y == self.size.height - 1 {
                        if x == 0 {
                            cell.character = '└';
                            cell.foreground_color = border_color;
                        } else if x == self.size.width - 1 {
                            cell.character = '┘';
                            cell.foreground_color = border_color;
                        } else {
                            cell.character = '─';
                            cell.foreground_color = border_color;
                        }
                    } else if x == 0 || x == self.size.width - 1 {
                        cell.character = '│';
                        cell.foreground_color = border_color;
                        cell.background_color = Color::Black;
                    } else {
                        continue;
                    }
                }
            }
        }

        let border_offset = self.border_offset();

        if let Some(title) = &self.title {
            for (x, c) in title.chars().enumerate() {
                let position = Vector2::new(x + 2, 0);

                if position.x >= self.size.width - border_offset {
                    break;
                }

                let cell = self.cell_mut(buffer, position);
                cell.character = c
            }
        }
    }

    pub fn set_border(&mut self, color: Option<Color>) {
        self.border_color = color
    }

    pub fn set_title(&mut self, title: Option<String>) {
        self.title = title;
    }
}

#[derive(Clone, Copy, Default)]
pub enum HorizontalAlignment {
    #[default]
    Left,
    Right,
    Center,
}

#[derive(Clone, Copy, Default)]
pub enum VerticalAlignment {
    #[default]
    Top,
    Bottom,
    Center,
}

#[derive(Copy, Clone)]
pub struct Cell {
    character: char,
    foreground_color: Color,
    background_color: Color,
}

impl Default for Cell {
    fn default() -> Self {
        Cell {
            character: ' ',
            foreground_color: Color::Default,
            background_color: Color::Default,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Color {
    // User's terminal default color
    Black,
    Cyan,
    Default,
    Green,
}

impl Color {
    fn apply_foreground(&self) {
        match self {
            Color::Black => print!("\x1b[30m"),
            Color::Cyan => print!("\x1b[36m"),
            Color::Default => print!("\x1b[39m"),
            Color::Green => print!("\x1b[32m"),
        }
    }

    fn apply_background(&self) {
        match self {
            Color::Black => print!("\x1b[40m"),
            Color::Cyan => print!("\x1b[46m"),
            Color::Default => print!("\x1b[49m"),
            Color::Green => print!("\x1b[42m"),
        }
    }
}

struct HardwrappingText<'a> {
    text: &'a [char],
    width: usize,
}

impl<'a> HardwrappingText<'a> {
    pub fn new(text: &'a [char], width: usize) -> Self {
        Self { text, width }
    }
}

impl<'a> Iterator for HardwrappingText<'a> {
    type Item = &'a [char];

    fn next(&mut self) -> Option<Self::Item> {
        if self.text.is_empty() {
            return None;
        }

        let mut found_newline = false;
        let line_end = match self.text.iter().position(|c| c == &'\n') {
            Some(position) => {
                found_newline = true;
                position
            }
            None => self.text.len(),
        };

        // FIXME: Account for word boundaries

        // We do not want to print the '\n' but we do want to remove it from the buffer so we can
        // parse the next line later, otherwise it gets stuck
        let strip_newline = found_newline & (line_end <= self.width);
        let hardwrapped_line_end = usize::min(self.width, line_end);

        let result = &self.text[0..hardwrapped_line_end];
        self.text = &self.text[hardwrapped_line_end + strip_newline as usize..];

        Some(result)
    }
}

#[derive(Default)]
pub struct Text {
    text: Vec<char>,
    rendering_region: RenderingRegion,
}

implement_common_widget!(Text);

impl Text {
    pub fn new(rendering_region: RenderingRegion) -> Text {
        Text {
            rendering_region,
            ..Default::default()
        }
    }

    pub fn set_text(&mut self, new_text: Option<String>) {
        if let Some(text) = new_text {
            // If not removed the tabs will be rendered as multiple spaces but the renderer will
            // count only one character, breaking the UI
            let text = text.replace('\t', "    ");
            // Some unicode characters are not rendered, breaking the UI
            // TODO: Find a scalable way to keep only "printable" characters
            self.text = text.chars().filter(|c| *c != '\u{300}').collect();
        } else {
            self.text.clear();
        }
    }
}

impl Widget for Text {
    fn render(&self, buffer: &mut Buffer) {
        let lines_count =
            HardwrappingText::new(&self.text, self.rendering_region.usable_size().width).count();

        let y_offset = self.rendering_region.vertical_offset(lines_count);

        for (line_index, line) in
            HardwrappingText::new(&self.text, self.rendering_region.usable_size().width)
                .take(self.rendering_region.usable_size().height)
                .enumerate()
        {
            let line_length = line.len();

            let x_offset = self.rendering_region.horizontal_offset(line_length);

            for (row_index, c) in line.iter().enumerate() {
                let cell = self.rendering_region.cell_mut(
                    buffer,
                    Vector2::new(row_index + x_offset, line_index + y_offset),
                );

                cell.character = *c;
            }
        }

        self.rendering_region.render(buffer);
    }
}

#[derive(Default)]
pub struct ItemList {
    items: Vec<String>,
    rendering_region: RenderingRegion,
    selected_row: Option<usize>,
}

implement_common_widget!(ItemList);

impl ItemList {
    pub fn new(rendering_region: RenderingRegion) -> ItemList {
        ItemList {
            rendering_region,
            ..Default::default()
        }
    }

    pub fn get_items_mut(&mut self) -> &mut Vec<String> {
        &mut self.items
    }

    pub fn change_list(&mut self, items: Vec<String>) {
        let inner_size = self.rendering_region.usable_size();

        assert!(items.len() <= inner_size.height);
        assert!(items.iter().map(|item| item.len()).max() < Some(inner_size.width));

        self.items = items;
        self.selected_row = None;
    }

    pub fn set_selected(&mut self, item_index: Option<usize>) {
        self.selected_row = item_index
    }
}

impl Widget for ItemList {
    fn render(&self, buffer: &mut Buffer) {
        let y_offset = self.rendering_region.vertical_offset(self.items.len());
        let x_offset = self
            .rendering_region
            .horizontal_offset(self.items.iter().map(|item| item.len()).max().unwrap_or(0));

        if let Some(selected_row) = self.selected_row {
            self.rendering_region
                .highlight_row(buffer, y_offset + selected_row)
        }

        for (y, item) in self.items.iter().enumerate() {
            for (x, c) in item.chars().enumerate() {
                let cell = self
                    .rendering_region
                    .cell_mut(buffer, Vector2::new(x + x_offset, y + y_offset));

                cell.character = c;
            }
        }

        self.rendering_region.render(buffer);
    }
}

#[derive(Default)]
pub struct Table {
    items: Vec<Vec<String>>,
    rendering_region: RenderingRegion,
    selected_row: Option<usize>,
}

implement_common_widget!(Table);

impl Table {
    pub fn new(rendering_region: RenderingRegion) -> Table {
        Table {
            rendering_region,
            ..Default::default()
        }
    }

    pub fn set_selected(&mut self, row_index: Option<usize>) {
        self.selected_row = row_index
    }

    pub fn change_table(&mut self, items: Vec<Vec<String>>) {
        self.items = items;
        self.selected_row = None;
    }
}

impl Widget for Table {
    fn render(&self, buffer: &mut Buffer) {
        let usable_size = self.rendering_region.usable_size();

        let max_row_size = self.items.iter().map(|row| row.len()).max().unwrap();

        let mut column_lengths = vec![0; max_row_size];
        for row in self.items.iter() {
            for (i, item) in row.iter().enumerate() {
                if item.len() > column_lengths[i] {
                    column_lengths[i] = item.len();
                }
            }
        }

        let y_offset = self.rendering_region.vertical_offset(self.items.len());

        if let Some(selected_row) = self.selected_row {
            self.rendering_region
                .highlight_row(buffer, y_offset + selected_row)
        }

        for (row_index, row) in self.items.iter().enumerate() {
            'line: for (column_index, item) in row.iter().enumerate() {
                let column_offset = column_lengths.iter().take(column_index).sum::<usize>();
                let x_offset = self.rendering_region.horizontal_offset(item.len());

                for (k, c) in item.chars().enumerate() {
                    // We sum the 'column_index' to add gaps
                    let x = column_index + k + column_offset;

                    // This truncates the line to avoid leaving the rendering area
                    if x >= usable_size.width {
                        break 'line;
                    }

                    let cell = self
                        .rendering_region
                        .cell_mut(buffer, Vector2::new(x + x_offset, row_index + y_offset));

                    cell.character = c;
                }
            }
        }
        self.rendering_region.render(buffer);
    }
}

// TODO: Add diff-rendering instead of clearing and rendering everything back again on every tick
// TODO: Add floating panel
// TODO: Can we get away with '&str' instead of 'String' everywhere in the Tui?
// TODO: Handle resizes
// TODO: Add tests with expectations
// TODO: Add manual libc binding
