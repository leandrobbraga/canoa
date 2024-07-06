//! Minimal terminal user interface (TUI) implementation.
//! It's inspired in the tiling window manager system, where the user always have the whole screen
//! covered and it just splits it between different widgets.

use std::io::{stdout, Write};

// TODO: Introduce the concept of scrooling, both vertical and horizontal
// TODO: Add wrap-around/truncate option to text, including in lists and tables instead of panicking
// TODO: Add diff-rendering instead of clearing and rendering everything back again on every tick
// TODO: Add floating panel
pub trait Widget {
    fn render(&self, terminal: &mut Terminal);
    fn height(&self) -> usize;
    fn width(&self) -> usize;

    fn set_border_color(&mut self, color: Color);
    fn set_title(&mut self, title: Option<String>);

    // TODO: Add methods for inner height and width for content rendering.
}

#[derive(Copy, Clone)]
struct Cell {
    character: char,
    foreground_color: Color,
    background_color: Color,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Color {
    // User's terminal default color
    Default,
    Green,
    Cyan,
    Black,
}

impl Color {
    fn apply_foreground(&self) {
        match self {
            Color::Green => print!("\x1b[32m"),
            Color::Cyan => print!("\x1b[36m"),
            Color::Default => print!("\x1b[39m"),
            Color::Black => print!("\x1b[30m"),
        }
    }

    fn apply_background(&self) {
        match self {
            Color::Green => print!("\x1b[42m"),
            Color::Cyan => print!("\x1b[46m"),
            Color::Default => print!("\x1b[49m"),
            Color::Black => print!("\x1b[40m"),
        }
    }
}

pub struct Terminal {
    buffer: Vec<Cell>,
    width: usize,
    height: usize,
}

impl Drop for Terminal {
    fn drop(&mut self) {
        Terminal::make_cursor_visible();
    }
}

impl Terminal {
    pub fn new() -> Terminal {
        Terminal::make_cursor_invisible();
        let (width, height) = Terminal::size().unwrap();

        Terminal {
            buffer: vec![
                Cell {
                    character: ' ',
                    foreground_color: Color::Default,
                    background_color: Color::Default
                };
                width * height
            ],
            width,
            height,
        }
    }

    pub fn flush(&self) {
        Terminal::clear_screen();

        // We always start with the Default color to ensure consistency
        let mut current_foreground_color = Color::Default;
        let mut current_background_color = Color::Default;
        current_foreground_color.apply_foreground();
        current_background_color.apply_background();

        for line in (0..self.buffer.len()).step_by(self.width) {
            print!("\n");

            for i in line..line + self.width {
                let cell = self.buffer[i];

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
        }

        stdout().flush().unwrap()
    }

    pub fn area(&self) -> Rectangle {
        Rectangle::new(None, 0, 0, self.width, self.height)
    }

    fn size() -> std::io::Result<(usize, usize)> {
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

            // FIXME: We are removing '-1' here because we're adding an extra println! at the end,
            // not showing the whole screen at once
            Ok((size.col as usize, size.row as usize))
        }
    }

    #[inline(always)]
    fn position_to_buffer_index(&self, x: usize, y: usize) -> usize {
        debug_assert!(x <= self.width);
        debug_assert!(y <= self.height);

        y * self.width + x
    }

    fn clear_screen() {
        print!("\x1b[2J");
    }

    fn make_cursor_invisible() {
        print!("\x1b[?25l");
    }

    fn make_cursor_visible() {
        print!("\x1b[?25h");
    }
}

pub struct Rectangle {
    title: Option<String>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    border_color: Color,
}

impl Rectangle {
    fn new(title: Option<String>, x: usize, y: usize, width: usize, height: usize) -> Rectangle {
        Rectangle {
            title,
            x,
            y,
            width,
            height,
            border_color: Color::Default,
        }
    }

    pub fn split_horizontally(self) -> (Rectangle, Rectangle) {
        self.split_horizontally_at(0.5)
    }

    /// Horizontal split
    /// +-----++-----+
    /// |     ||     |
    /// |     ||     |
    /// |     ||     |
    /// |     ||     |
    /// +-----++-----+
    pub fn split_horizontally_at(self, percentage: f32) -> (Rectangle, Rectangle) {
        assert!(percentage > 0.0 && percentage < 1.0);

        let left_width = (self.width as f32 * percentage) as usize;
        let right_width = self.width - left_width;

        let left = Rectangle {
            title: None,
            x: self.x,
            y: self.y,
            width: left_width,
            height: self.height,
            border_color: self.border_color,
        };
        let right = Rectangle {
            title: None,
            x: self.x + left_width,
            y: self.y,
            width: right_width,
            height: self.height,
            border_color: self.border_color,
        };

        (left, right)
    }

    pub fn split_vertically(self) -> (Rectangle, Rectangle) {
        self.split_vertically_at(0.5)
    }

    /// Vertical split
    /// +------------+
    /// |            |
    /// +------------+
    /// +------------+
    /// |            |
    /// +------------+
    pub fn split_vertically_at(self, percentage: f32) -> (Rectangle, Rectangle) {
        assert!(percentage > 0.0 && percentage < 1.0);

        let top_height = (self.height as f32 * percentage) as usize;
        let bottom_height = self.height - top_height;

        let top = Rectangle {
            title: None,
            x: self.x,
            y: self.y,
            width: self.width,
            height: top_height,
            border_color: self.border_color,
        };
        let bottom = Rectangle {
            title: None,
            x: self.x,
            y: self.y + top_height,
            width: self.width,
            height: bottom_height,
            border_color: self.border_color,
        };

        (top, bottom)
    }

    pub fn text(
        self,
        text: String,
        vertical_alignment: VerticalAlignment,
        horizontal_alignment: HorizontalAlignment,
    ) -> Text {
        Text::new(text, vertical_alignment, horizontal_alignment, self)
    }

    pub fn item_list(
        self,
        items: Vec<String>,
        vertical_alignment: VerticalAlignment,
        horizontal_alignment: HorizontalAlignment,
    ) -> ItemList {
        ItemList::new(items, vertical_alignment, horizontal_alignment, self)
    }

    pub fn table(
        self,
        items: Vec<Vec<String>>,
        vertical_alignment: VerticalAlignment,
        horizontal_alignment: HorizontalAlignment,
    ) -> Table {
        Table::new(items, vertical_alignment, horizontal_alignment, self)
    }

    #[inline(always)]
    fn position_to_buffer_index(&self, terminal: &Terminal, x: usize, y: usize) -> usize {
        debug_assert!(x <= self.width);
        debug_assert!(y <= self.height);

        terminal.position_to_buffer_index(self.x + x, self.y + y)
    }
}

impl Widget for Rectangle {
    fn render(&self, terminal: &mut Terminal) {
        // We iterate in this order to help with cache locality
        for y in 0..self.height {
            for x in 0..self.width {
                let buffer_index = self.position_to_buffer_index(terminal, x, y);

                if y == 0 {
                    if x == 0 {
                        terminal.buffer[buffer_index].character = '┌';
                        terminal.buffer[buffer_index].foreground_color = self.border_color;
                    } else if x == self.width - 1 {
                        terminal.buffer[buffer_index].character = '┐';
                        terminal.buffer[buffer_index].foreground_color = self.border_color;
                    } else {
                        terminal.buffer[buffer_index].character = '─';
                        terminal.buffer[buffer_index].foreground_color = self.border_color;
                    }
                } else if y == self.height - 1 {
                    if x == 0 {
                        terminal.buffer[buffer_index].character = '└';
                        terminal.buffer[buffer_index].foreground_color = self.border_color;
                    } else if x == self.width - 1 {
                        terminal.buffer[buffer_index].character = '┘';
                        terminal.buffer[buffer_index].foreground_color = self.border_color;
                    } else {
                        terminal.buffer[buffer_index].character = '─';
                        terminal.buffer[buffer_index].foreground_color = self.border_color;
                    }
                } else if x == 0 || x == self.width - 1 {
                    terminal.buffer[buffer_index].character = '│';
                    terminal.buffer[buffer_index].foreground_color = self.border_color;
                } else {
                    continue;
                }
            }
        }

        if let Some(title) = &self.title {
            for (x, c) in title.chars().enumerate() {
                let buffer_index = self.position_to_buffer_index(terminal, x + 2, 0);
                terminal.buffer[buffer_index].character = c
            }
        }
    }

    fn height(&self) -> usize {
        self.height
    }

    fn width(&self) -> usize {
        self.width
    }

    fn set_border_color(&mut self, color: Color) {
        self.border_color = color
    }

    fn set_title(&mut self, title: Option<String>) {
        self.title = title;
    }
}

pub struct Text {
    text: String,
    vertical_alignment: VerticalAlignment,
    horizontal_alignment: HorizontalAlignment,
    area: Rectangle,
    lines_count: usize,
}

pub enum HorizontalAlignment {
    Left,
    Right,
    Center,
}

pub enum VerticalAlignment {
    Top,
    Bottom,
    Center,
}

impl Text {
    fn new(
        text: String,
        vertical_alignment: VerticalAlignment,
        horizontal_alignment: HorizontalAlignment,
        area: Rectangle,
    ) -> Text {
        // FIXME: Deal with hardwrap
        let lines_count = text.chars().filter(|c| *c == '\n').count();

        assert!(lines_count < area.height - 2);

        Text {
            text,
            vertical_alignment,
            horizontal_alignment,
            area,
            lines_count,
        }
    }
}
impl Widget for Text {
    fn render(&self, terminal: &mut Terminal) {
        self.area.render(terminal);

        let y = match self.vertical_alignment {
            VerticalAlignment::Top => 1, // 1 for the border
            VerticalAlignment::Bottom => self.height() - 1 - 1 - self.lines_count, // -1 for the border
            VerticalAlignment::Center => (self.height() - self.lines_count) / 2,
        };

        for (line_index, line) in self.text.lines().enumerate() {
            let line_lenght = line.len();

            let x = match self.horizontal_alignment {
                HorizontalAlignment::Left => 1, // 1 for the border
                HorizontalAlignment::Right => {
                    self.width() - line_lenght - 1 // -1 for the border
                }
                HorizontalAlignment::Center => (self.width() - line_lenght) / 2,
            };

            // FIXME: Deal with hardwrap
            for (row_index, c) in line.chars().enumerate() {
                let buffer_index =
                    self.area
                        .position_to_buffer_index(terminal, x + row_index, y + line_index);

                terminal.buffer[buffer_index].character = c;
            }
        }
    }

    fn height(&self) -> usize {
        self.area.height
    }

    fn width(&self) -> usize {
        self.area.width
    }

    fn set_border_color(&mut self, color: Color) {
        self.area.set_border_color(color)
    }

    fn set_title(&mut self, title: Option<String>) {
        self.area.set_title(title);
    }
}

pub struct ItemList {
    items: Vec<String>,
    vertical_alignment: VerticalAlignment,
    horizontal_alignment: HorizontalAlignment,
    area: Rectangle,
    selected_row: Option<usize>,
}

impl ItemList {
    fn new(
        items: Vec<String>,
        vertical_alignment: VerticalAlignment,
        horizontal_alignment: HorizontalAlignment,
        area: Rectangle,
    ) -> ItemList {
        assert!(items.len() <= area.height - 2); // -2 for the border
        assert!(items.iter().map(|item| item.len()).max() < Some(area.width - 2)); // -2 for the border

        ItemList {
            items,
            vertical_alignment,
            horizontal_alignment,
            area,
            selected_row: None,
        }
    }

    pub fn set_selected(&mut self, item_index: Option<usize>) {
        self.selected_row = item_index
    }
}

impl Widget for ItemList {
    fn render(&self, terminal: &mut Terminal) {
        self.area.render(terminal);

        // Fast path, there is nothing to render
        if self.items.is_empty() {
            return;
        }

        let y_offset = match self.vertical_alignment {
            VerticalAlignment::Top => 1, // 1 for the border
            VerticalAlignment::Bottom => self.area.height - self.items.len() - 1, // -1 for the border
            VerticalAlignment::Center => (self.area.height - self.items.len()) / 2,
        };

        let x_offset = match self.horizontal_alignment {
            HorizontalAlignment::Left => 1, // 1 for the border
            HorizontalAlignment::Right => {
                self.area.width - self.items.iter().map(|item| item.len()).max().unwrap_or(0) - 1
                // -1 for the border
            }
            HorizontalAlignment::Center => {
                (self.area.width - self.items.iter().map(|item| item.len()).max().unwrap_or(0)) / 2
            }
        };

        if let Some(selected_row) = self.selected_row {
            for i in 1..self.width() - 1 {
                let buffer_index =
                    self.area
                        .position_to_buffer_index(terminal, i, y_offset + selected_row);

                terminal.buffer[buffer_index].background_color = Color::Cyan;
                terminal.buffer[buffer_index].foreground_color = Color::Black;
            }
        }

        for (y, item) in self.items.iter().enumerate() {
            for (x, c) in item.chars().enumerate() {
                let buffer_index =
                    self.area
                        .position_to_buffer_index(terminal, x_offset + x, y_offset + y);
                terminal.buffer[buffer_index].character = c;
            }
        }
    }

    fn height(&self) -> usize {
        self.area.height
    }

    fn width(&self) -> usize {
        self.area.width
    }

    fn set_border_color(&mut self, color: Color) {
        self.area.set_border_color(color)
    }

    fn set_title(&mut self, title: Option<String>) {
        self.area.set_title(title);
    }
}

pub struct Table {
    items: Vec<Vec<String>>,
    vertical_alignment: VerticalAlignment,
    horizontal_alignment: HorizontalAlignment,
    area: Rectangle,
    column_lengths: Vec<usize>,
    selected_row: Option<usize>,
}

impl Table {
    fn new(
        items: Vec<Vec<String>>,
        vertical_alignment: VerticalAlignment,
        horizontal_alignment: HorizontalAlignment,
        area: Rectangle,
    ) -> Table {
        let max_row_size = items.iter().map(|row| row.len()).max().unwrap();

        let mut column_lengths = vec![0; max_row_size];
        for row in items.iter() {
            for (i, item) in row.iter().enumerate() {
                if item.len() > column_lengths[i] {
                    column_lengths[i] = item.len();
                }
            }
        }

        let required_width: usize = column_lengths.iter().sum();

        assert!((items.len()) <= area.height - 2); // -2 for the border
        assert!(required_width < area.width - 2); // -2 for the border

        Table {
            items,
            vertical_alignment,
            horizontal_alignment,
            area,
            column_lengths,
            selected_row: None,
        }
    }

    pub fn set_selected(&mut self, row_index: Option<usize>) {
        self.selected_row = row_index
    }
}

impl Widget for Table {
    fn render(&self, terminal: &mut Terminal) {
        self.area.render(terminal);

        // Fast path, there is nothing to render
        if self.items.is_empty() {
            return;
        }

        let y_offset = match self.vertical_alignment {
            VerticalAlignment::Top => 1, // 1 for the border
            VerticalAlignment::Bottom => self.area.height - self.items.len() - 1, // -1 for the border
            VerticalAlignment::Center => (self.area.height - self.items.len()) / 2,
        };

        let x_offset = match self.horizontal_alignment {
            HorizontalAlignment::Left => 1, // 1 for the border
            HorizontalAlignment::Right => {
                // -1 for the border
                self.area.width
                    - self.column_lengths.iter().sum::<usize>()
                    - 1
                    // For the spacing between columns
                    - self.column_lengths.len() - 1
            }
            HorizontalAlignment::Center => {
                (self.area.width
                    - self.column_lengths.iter().sum::<usize>()
                    // For the spacing between columns
                    - self.column_lengths.len()
                    - 1)
                    / 2
            }
        };

        if let Some(selected_row) = self.selected_row {
            for i in 1..self.width() - 1 {
                let buffer_index =
                    self.area
                        .position_to_buffer_index(terminal, i, y_offset + selected_row);

                terminal.buffer[buffer_index].background_color = Color::Cyan;
                terminal.buffer[buffer_index].foreground_color = Color::Black;
            }
        }

        for (row_index, row) in self.items.iter().enumerate() {
            for (column_index, item) in row.iter().enumerate() {
                for (k, c) in item.chars().enumerate() {
                    // We sum the 'column_index' in the end to add gaps
                    let x =
                        self.column_lengths.iter().take(column_index).sum::<usize>() + column_index;

                    let buffer_index = self.area.position_to_buffer_index(
                        terminal,
                        x_offset + x + k,
                        y_offset + row_index,
                    );
                    terminal.buffer[buffer_index].character = c;
                }
            }
        }
    }

    fn height(&self) -> usize {
        self.area.height
    }

    fn width(&self) -> usize {
        self.area.width
    }

    fn set_border_color(&mut self, color: Color) {
        self.area.set_border_color(color)
    }

    fn set_title(&mut self, title: Option<String>) {
        self.area.set_title(title);
    }
}
