//! Minimal terminal user interface (TUI) implementation.
//! It's inspired in the tiling window manager system, where the user always have the whole screen
//! covered and it just splits it between different widgets.

use std::io::{stdout, Read, Write};
use std::{mem::MaybeUninit, os::fd::AsRawFd};

use libc::termios as Termios;

// TODO: Introduce the concept of vertical scrolling
// TODO: Add diff-rendering instead of clearing and rendering everything back again on every tick
// TODO: Add floating panel
// TODO: Can we get away with '&str' instead of 'String' everywhere in the Tui?
// TODO: Handle resizes
pub trait Widget {
    fn render(&self, terminal: &mut Terminal);
    fn height(&self) -> usize;
    fn width(&self) -> usize;

    fn set_border_color(&mut self, color: Color);
    fn set_title(&mut self, title: Option<String>);

    // TODO: Add methods for inner height and width for content rendering.
}

pub struct Terminal {
    buffer: Vec<Cell>,
    width: usize,
    height: usize,

    tty: std::fs::File,
    termios: Termios,
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if let Err(err) = self.disable_raw_mode() {
            eprintln!("ERROR: Could not return the terminal to canonical mode, run 'reset' to force it back: {err}")
        };

        Terminal::make_cursor_visible();
    }
}

impl Terminal {
    pub fn try_new() -> std::io::Result<Terminal> {
        let tty = std::fs::File::open("/dev/tty")?;

        let termios = Terminal::init_termios(&tty)?;
        let (width, height) = Terminal::size().unwrap();

        let terminal = Terminal {
            buffer: vec![Cell::default(); width * height],
            width,
            height,
            tty,
            termios,
        };

        terminal.enable_raw_mode()?;

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
        Terminal::clear_screen();

        // We always start with the Default color to ensure consistency
        let mut current_foreground_color = Color::Default;
        let mut current_background_color = Color::Default;
        current_foreground_color.apply_foreground();
        current_background_color.apply_background();

        for line in (0..self.buffer.len()).step_by(self.width) {
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

        stdout().flush().unwrap();
        self.buffer.fill(Cell::default())
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

    pub fn tty(&self) -> std::io::Result<std::io::Bytes<std::fs::File>> {
        self.tty.try_clone().map(|file| file.bytes())
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
    text: Vec<char>,
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
        let text: Vec<char> = text.chars().collect();
        let lines_count = HardwrappingText::new(&text, area.width() - 2)
            .into_iter()
            .count();

        Text {
            text,
            vertical_alignment,
            horizontal_alignment,
            area,
            lines_count,
        }
    }

    pub fn change_text(&mut self, new_text: Option<String>) {
        if let Some(text) = new_text {
            self.text = text.chars().collect();
        } else {
            self.text.clear();
        }

        self.lines_count = HardwrappingText::new(&self.text, self.area.width() - 2)
            .into_iter()
            .count();
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

        let hardwrapped_lines = HardwrappingText::new(&self.text, self.width() - 2);
        for (line_index, line) in hardwrapped_lines
            .into_iter()
            // FIXME: Deal with scrolling
            .take(self.height() - 2)
            .enumerate()
        {
            let x = match self.horizontal_alignment {
                HorizontalAlignment::Left => 1, // 1 for the border
                HorizontalAlignment::Right => {
                    self.width() - line.len() - 1 // -1 for the border
                }
                HorizontalAlignment::Center => (self.width() - line.len()) / 2,
            };

            for (row_index, c) in line.iter().enumerate() {
                let buffer_index =
                    self.area
                        .position_to_buffer_index(terminal, x + row_index, y + line_index);

                terminal.buffer[buffer_index].character = *c;
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

#[derive(Copy, Clone)]
struct Cell {
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

// TODO: Add tests with expectations
