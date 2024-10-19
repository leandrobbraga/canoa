//! Minimal terminal user interface (TUI) implementation.
//! It's inspired in the tiling window manager system, where the user always have the whole screen
//! covered and it just splits it between different widgets.

use std::io::{stdout, Read, Write};
use std::ops::{Add, AddAssign};
use std::{mem::MaybeUninit, os::fd::AsRawFd};

use libc::termios as Termios;

// TODO: Introduce the concept of vertical scrolling
// TODO: Add diff-rendering instead of clearing and rendering everything back again on every tick
// TODO: Add floating panel
// TODO: Can we get away with '&str' instead of 'String' everywhere in the Tui?
// TODO: Handle resizes
pub trait Widget {
    fn render(&self, buffer: &mut Buffer);
    fn size(&self) -> Size;
    fn inner_size(&self) -> Size;

    fn set_border_color(&mut self, color: Color);
    fn set_title(&mut self, title: Option<String>);

    fn rendering_region(self) -> RenderingRegion;
    // TODO: Add methods for inner height and width for content rendering.
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
            eprintln!("ERROR: Could not return the terminal to canonical mode, run 'reset' to force it back: {err}")
        };

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

        RenderingRegion::new(None, Vector2::default(), size)
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

#[derive(Clone, Copy)]
pub struct Size {
    pub width: usize,
    pub height: usize,
}

impl Size {
    fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }
}

pub struct RenderingRegion {
    title: Option<String>,
    position: Vector2,
    size: Size,
    border_color: Color,
}

impl RenderingRegion {
    fn new(title: Option<String>, position: Vector2, size: Size) -> RenderingRegion {
        RenderingRegion {
            title,
            position,
            size,
            border_color: Color::Default,
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
        self.split_vertically_at(0.5)
    }

    pub fn split_vertically_at(self, percentage: f32) -> (RenderingRegion, RenderingRegion) {
        assert!(percentage > 0.0 && percentage < 1.0);

        let left_width = (self.size.width as f32 * percentage) as usize;
        let right_width = self.size.width - left_width;

        let left = RenderingRegion {
            title: None,
            position: self.position,
            size: Size::new(left_width, self.size.height),
            border_color: self.border_color,
        };
        let right = RenderingRegion {
            title: None,
            position: self.position + Vector2::new(left_width, 0),
            size: Size::new(right_width, self.size.height),
            border_color: self.border_color,
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
    pub fn split_hotizontally(self) -> (RenderingRegion, RenderingRegion) {
        self.split_hotizontally_at(0.5)
    }

    pub fn split_hotizontally_at(self, percentage: f32) -> (RenderingRegion, RenderingRegion) {
        assert!(percentage > 0.0 && percentage < 1.0);

        let top_height = (self.size.height as f32 * percentage) as usize;
        let bottom_height = self.size.height - top_height;

        let top = RenderingRegion {
            title: None,
            position: self.position,
            size: Size::new(self.size.width, top_height),
            border_color: self.border_color,
        };
        let bottom = RenderingRegion {
            title: None,
            position: self.position + Vector2::new(0, top_height),
            size: Size::new(self.size.width, bottom_height),
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
    fn cell_mut<'a>(&self, buffer: &'a mut Buffer, position: Vector2) -> &'a mut Cell {
        debug_assert!(position.x <= self.size.width);
        debug_assert!(position.y <= self.size.height);

        buffer.cell_mut(self.position + position)
    }

    fn highlight_row(&self, buffer: &mut Buffer, selected_row: usize) {
        // We avoid highlighting the borders
        for column in 1..self.size.width - 1 {
            let cell = self.cell_mut(buffer, Vector2::new(column, selected_row));

            cell.background_color = Color::Cyan;
            cell.foreground_color = Color::Black;
        }
    }

    fn render(&self, buffer: &mut Buffer) {
        // Render the border
        for y in 0..self.size.height {
            for x in 0..self.size.width {
                let cell = self.cell_mut(buffer, Vector2::new(x, y));

                if y == 0 {
                    if x == 0 {
                        cell.character = '┌';
                        cell.foreground_color = self.border_color;
                    } else if x == self.size.width - 1 {
                        cell.character = '┐';
                        cell.foreground_color = self.border_color;
                    } else {
                        cell.character = '─';
                        cell.foreground_color = self.border_color;
                    }
                } else if y == self.size.height - 1 {
                    if x == 0 {
                        cell.character = '└';
                        cell.foreground_color = self.border_color;
                    } else if x == self.size.width - 1 {
                        cell.character = '┘';
                        cell.foreground_color = self.border_color;
                    } else {
                        cell.character = '─';
                        cell.foreground_color = self.border_color;
                    }
                } else if x == 0 || x == self.size.width - 1 {
                    cell.character = '│';
                    cell.foreground_color = self.border_color;
                } else {
                    continue;
                }
            }
        }

        // Render the title
        if let Some(title) = &self.title {
            for (x, c) in title.chars().enumerate() {
                let position = Vector2::new(x + 2, 0);

                if position.x >= self.inner_size().width - 1 {
                    break;
                }

                let cell = self.cell_mut(buffer, position);
                cell.character = c
            }
        }
    }

    /// The inner size of the rendering area, discards the border
    pub fn inner_size(&self) -> Size {
        Size {
            width: self.size.width - 2,
            height: self.size.height - 2,
        }
    }

    fn set_border_color(&mut self, color: Color) {
        self.border_color = color
    }

    pub fn set_title(&mut self, title: Option<String>) {
        self.title = title;
    }
}

pub struct Text {
    text: Vec<char>,
    vertical_alignment: VerticalAlignment,
    horizontal_alignment: HorizontalAlignment,
    rendering_region: RenderingRegion,
    y_offset: usize,
}

#[derive(Clone, Copy)]
pub enum HorizontalAlignment {
    Left,
    Right,
    Center,
}

#[derive(Clone, Copy)]
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
        rendering_region: RenderingRegion,
    ) -> Text {
        let text: Vec<char> = text.chars().collect();

        let y_offset = Text::calculate_y_offset(&text, rendering_region.size, vertical_alignment);

        Text {
            text,
            vertical_alignment,
            horizontal_alignment,
            rendering_region,
            y_offset,
        }
    }

    fn calculate_y_offset(
        text: &[char],
        size: Size,
        vertical_alignment: VerticalAlignment,
    ) -> usize {
        let lines_count = HardwrappingText::new(text, size.width).into_iter().count();

        match vertical_alignment {
            VerticalAlignment::Top => 1, // 1 for the border
            VerticalAlignment::Bottom => size.height - 1 - 1 - lines_count, // -1 for the border
            VerticalAlignment::Center => (size.height - lines_count) / 2,
        }
    }

    pub fn change_text(&mut self, new_text: Option<String>) {
        if let Some(text) = new_text {
            // If not removed the tabs will be rendered as multiple spaces but the renderer will
            // count only one character, breaking the UI
            let text = text.replace('\t', "    ");
            // Some unicode characters are not rendered, breaking the UI
            // TODO: Find a scalable way to keep only "printable" characters
            self.text = text.chars().filter(|c| !(*c == '\u{300}')).collect();
        } else {
            self.text.clear();
        }

        self.y_offset = Text::calculate_y_offset(
            &self.text,
            self.rendering_region.size,
            self.vertical_alignment,
        );
    }
}
impl Widget for Text {
    fn rendering_region(self) -> RenderingRegion {
        self.rendering_region
    }

    fn render(&self, buffer: &mut Buffer) {
        self.rendering_region.render(buffer);

        for (line_index, line) in
            HardwrappingText::new(&self.text, self.rendering_region.inner_size().width)
                .into_iter()
                // FIXME: Deal with scrolling
                .take(self.rendering_region.inner_size().height)
                .enumerate()
        {
            let x_offset = match self.horizontal_alignment {
                HorizontalAlignment::Left => 1, // 1 for the border
                HorizontalAlignment::Right => {
                    self.rendering_region.size.width - line.len() - 1 // -1 for the border
                }
                HorizontalAlignment::Center => (self.rendering_region.size.width - line.len()) / 2,
            };

            for (row_index, c) in line.iter().enumerate() {
                let cell = self.rendering_region.cell_mut(
                    buffer,
                    Vector2::new(row_index, line_index) + Vector2::new(x_offset, self.y_offset),
                );

                cell.character = *c;
            }
        }
    }

    fn size(&self) -> Size {
        self.rendering_region.size
    }

    fn inner_size(&self) -> Size {
        self.rendering_region.inner_size()
    }

    fn set_border_color(&mut self, color: Color) {
        self.rendering_region.set_border_color(color)
    }

    fn set_title(&mut self, title: Option<String>) {
        self.rendering_region.set_title(title);
    }
}

pub struct ItemList {
    items: Vec<String>,
    rendering_region: RenderingRegion,
    selected_row: Option<usize>,
    offset: Vector2,
    vertical_alignment: VerticalAlignment,
    horizontal_alignment: HorizontalAlignment,
}

impl ItemList {
    fn new(
        items: Vec<String>,
        vertical_alignment: VerticalAlignment,
        horizontal_alignment: HorizontalAlignment,
        rendering_region: RenderingRegion,
    ) -> ItemList {
        let inner_size = rendering_region.inner_size();

        assert!(items.len() <= inner_size.height);
        assert!(items.iter().map(|item| item.len()).max() < Some(inner_size.width));

        let offset = {
            let y_offset = match vertical_alignment {
                VerticalAlignment::Top => 1, // 1 for the border
                VerticalAlignment::Bottom => rendering_region.size.height - items.len() - 1, // -1 for the border
                VerticalAlignment::Center => (rendering_region.size.height - items.len()) / 2,
            };

            let x_offset = match horizontal_alignment {
                HorizontalAlignment::Left => 1, // 1 for the border
                HorizontalAlignment::Right => {
                    rendering_region.size.width
                        - items.iter().map(|item| item.len()).max().unwrap_or(0)
                        - 1
                    // -1 for the border
                }
                HorizontalAlignment::Center => {
                    (rendering_region.size.width
                        - items.iter().map(|item| item.len()).max().unwrap_or(0))
                        / 2
                }
            };

            Vector2::new(x_offset, y_offset)
        };

        ItemList {
            items,
            rendering_region,
            selected_row: None,
            offset,
            vertical_alignment,
            horizontal_alignment,
        }
    }

    // TODO: Remove duplicate code
    pub fn change_list(&mut self, items: Vec<String>) {
        let inner_size = self.rendering_region.inner_size();

        assert!(items.len() <= inner_size.height);
        assert!(items.iter().map(|item| item.len()).max() < Some(inner_size.width));

        let offset = {
            let y_offset = match self.vertical_alignment {
                VerticalAlignment::Top => 1, // 1 for the border
                VerticalAlignment::Bottom => self.rendering_region.size.height - items.len() - 1, // -1 for the border
                VerticalAlignment::Center => (self.rendering_region.size.height - items.len()) / 2,
            };

            let x_offset = match self.horizontal_alignment {
                HorizontalAlignment::Left => 1, // 1 for the border
                HorizontalAlignment::Right => {
                    self.rendering_region.size.width
                        - items.iter().map(|item| item.len()).max().unwrap_or(0)
                        - 1
                    // -1 for the border
                }
                HorizontalAlignment::Center => {
                    (self.rendering_region.size.width
                        - items.iter().map(|item| item.len()).max().unwrap_or(0))
                        / 2
                }
            };

            Vector2::new(x_offset, y_offset)
        };

        self.items = items;
        self.selected_row = None;
        self.offset = offset;
    }

    pub fn set_selected(&mut self, item_index: Option<usize>) {
        self.selected_row = item_index
    }
}

impl Widget for ItemList {
    fn rendering_region(self) -> RenderingRegion {
        self.rendering_region
    }

    fn render(&self, buffer: &mut Buffer) {
        self.rendering_region.render(buffer);

        // Fast path, there is nothing to render
        if self.items.is_empty() {
            return;
        }

        if let Some(selected_row) = self.selected_row {
            self.rendering_region
                .highlight_row(buffer, self.offset.y + selected_row)
        }

        for (y, item) in self.items.iter().enumerate() {
            for (x, c) in item.chars().enumerate() {
                let cell = self
                    .rendering_region
                    .cell_mut(buffer, Vector2::new(x, y) + self.offset);

                cell.character = c;
            }
        }
    }

    fn size(&self) -> Size {
        self.rendering_region.size
    }

    fn inner_size(&self) -> Size {
        self.rendering_region.inner_size()
    }

    fn set_border_color(&mut self, color: Color) {
        self.rendering_region.set_border_color(color)
    }

    fn set_title(&mut self, title: Option<String>) {
        self.rendering_region.set_title(title);
    }
}

pub struct Table {
    items: Vec<Vec<String>>,
    rendering_region: RenderingRegion,
    column_lengths: Vec<usize>,
    selected_row: Option<usize>,
    offset: Vector2,
    vertical_alignment: VerticalAlignment,
    horizontal_alignment: HorizontalAlignment,
}

impl Table {
    fn new(
        items: Vec<Vec<String>>,
        vertical_alignment: VerticalAlignment,
        horizontal_alignment: HorizontalAlignment,
        rendering_region: RenderingRegion,
    ) -> Table {
        let max_row_size = items.iter().map(|row| row.len()).max().unwrap_or(0);

        let mut column_lengths = vec![0; max_row_size];
        for row in items.iter() {
            for (i, item) in row.iter().enumerate() {
                if item.len() > column_lengths[i] {
                    column_lengths[i] = item.len();
                }
            }
        }

        let max_width = usize::max(
            column_lengths.iter().sum::<usize>() + column_lengths.len(),
            rendering_region.inner_size().width,
        );

        let inner_size = rendering_region.inner_size();
        assert!((items.len()) <= inner_size.height);

        let offset = {
            let y_offset = match vertical_alignment {
                VerticalAlignment::Top => 1, // 1 for the border
                VerticalAlignment::Bottom => rendering_region.size.height - items.len() - 1, // -1 for the border
                VerticalAlignment::Center => (rendering_region.size.height - items.len()) / 2,
            };

            let x_offset = match horizontal_alignment {
                HorizontalAlignment::Left => 1, // 1 for the border
                HorizontalAlignment::Right => {
                    // -1 for the border
                    rendering_region.size.width - max_width - 1
                }
                HorizontalAlignment::Center => (rendering_region.size.width - max_width) / 2,
            };

            Vector2::new(x_offset, y_offset)
        };

        Table {
            items,
            rendering_region,
            column_lengths,
            selected_row: None,
            offset,
            vertical_alignment,
            horizontal_alignment,
        }
    }

    pub fn set_selected(&mut self, row_index: Option<usize>) {
        self.selected_row = row_index
    }

    // TODO: Remove duplicate code
    pub fn change_table(&mut self, items: Vec<Vec<String>>) {
        let max_row_size = items.iter().map(|row| row.len()).max().unwrap();

        let mut column_lengths = vec![0; max_row_size];
        for row in items.iter() {
            for (i, item) in row.iter().enumerate() {
                if item.len() > column_lengths[i] {
                    column_lengths[i] = item.len();
                }
            }
        }

        let max_width = usize::max(
            column_lengths.iter().sum::<usize>() + column_lengths.len(),
            self.rendering_region.inner_size().width,
        );

        let inner_size = self.rendering_region.inner_size();
        assert!((items.len()) <= inner_size.height);

        let offset = {
            let y_offset = match self.vertical_alignment {
                VerticalAlignment::Top => 1, // 1 for the border
                VerticalAlignment::Bottom => self.rendering_region.size.height - items.len() - 1, // -1 for the border
                VerticalAlignment::Center => (self.rendering_region.size.height - items.len()) / 2,
            };

            let x_offset = match self.horizontal_alignment {
                HorizontalAlignment::Left => 1, // 1 for the border
                HorizontalAlignment::Right => {
                    // -1 for the border
                    self.rendering_region.size.width - max_width - 1
                }
                HorizontalAlignment::Center => (self.rendering_region.size.width - max_width) / 2,
            };

            Vector2::new(x_offset, y_offset)
        };

        self.items = items;
        self.column_lengths = column_lengths;
        self.selected_row = None;
        self.offset = offset;
    }
}

impl Widget for Table {
    fn rendering_region(self) -> RenderingRegion {
        self.rendering_region
    }

    fn render(&self, buffer: &mut Buffer) {
        self.rendering_region.render(buffer);

        // Fast path, there is nothing to render
        if self.items.is_empty() {
            return;
        }

        if let Some(selected_row) = self.selected_row {
            self.rendering_region
                .highlight_row(buffer, self.offset.y + selected_row)
        }

        for (row_index, row) in self.items.iter().enumerate() {
            'line: for (column_index, item) in row.iter().enumerate() {
                let column_offset = self.column_lengths.iter().take(column_index).sum::<usize>();

                for (k, c) in item.chars().enumerate() {
                    // We sum the 'column_index' in the end to add gaps
                    let x = column_offset + column_index + k;

                    // This truncates the line to avoid leaving the rendering area
                    if x >= self.inner_size().width {
                        break 'line;
                    }

                    let cell = self
                        .rendering_region
                        .cell_mut(buffer, Vector2::new(x, row_index) + self.offset);

                    cell.character = c;
                }
            }
        }
    }

    fn size(&self) -> Size {
        self.rendering_region.size
    }

    fn inner_size(&self) -> Size {
        self.rendering_region.inner_size()
    }

    fn set_border_color(&mut self, color: Color) {
        self.rendering_region.set_border_color(color)
    }

    fn set_title(&mut self, title: Option<String>) {
        self.rendering_region.set_title(title);
    }
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

// TODO: Add tests with expectations
// FIXME: There is a bug in the `Text` rendering which is rendering outside boundaries
