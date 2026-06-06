pub const SCREEN_WIDTH: usize = 800;
pub const SCREEN_HEIGHT: usize = 480;
pub const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
    Green = 2,
    Blue = 3,
    Red = 4,
    Yellow = 5,
}

impl Color {
    pub const fn ink_code(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScreenBuffer {
    pixels: [Color; SCREEN_PIXELS],
}

impl Default for ScreenBuffer {
    fn default() -> Self {
        Self::filled(Color::White)
    }
}

impl ScreenBuffer {
    pub fn filled(color: Color) -> Self {
        Self {
            pixels: [color; SCREEN_PIXELS],
        }
    }

    pub const fn width(&self) -> usize {
        SCREEN_WIDTH
    }

    pub const fn height(&self) -> usize {
        SCREEN_HEIGHT
    }

    pub fn pixels(&self) -> &[Color; SCREEN_PIXELS] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [Color; SCREEN_PIXELS] {
        &mut self.pixels
    }

    pub fn clear(&mut self, color: Color) {
        self.pixels.fill(color);
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> Option<Color> {
        self.pixel_index(x, y).map(|index| self.pixels[index])
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) -> bool {
        if let Some(index) = self.pixel_index(x, y) {
            self.pixels[index] = color;
            true
        } else {
            false
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: Color) {
        if width == 0 || height == 0 || x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT {
            return;
        }

        let end_x = x.saturating_add(width).min(SCREEN_WIDTH);
        let end_y = y.saturating_add(height).min(SCREEN_HEIGHT);

        for row in y..end_y {
            let start = row * SCREEN_WIDTH + x;
            let end = row * SCREEN_WIDTH + end_x;
            self.pixels[start..end].fill(color);
        }
    }

    fn pixel_index(&self, x: usize, y: usize) -> Option<usize> {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            Some(y * SCREEN_WIDTH + x)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayRefreshMode {
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayError {
    Busy,
    Transport,
    NotInitialized,
}

pub trait EpaperDisplay {
    fn init(&mut self) -> Result<(), DisplayError>;
    fn refresh(
        &mut self,
        buffer: &ScreenBuffer,
        mode: DisplayRefreshMode,
    ) -> Result<(), DisplayError>;
    fn sleep(&mut self) -> Result<(), DisplayError>;
}

#[derive(Clone, Debug)]
pub struct MockDisplay {
    initialized: bool,
    sleeping: bool,
    refresh_count: u32,
    last_refresh_mode: Option<DisplayRefreshMode>,
    last_frame: Option<ScreenBuffer>,
}

impl Default for MockDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl MockDisplay {
    pub const fn new() -> Self {
        Self {
            initialized: false,
            sleeping: false,
            refresh_count: 0,
            last_refresh_mode: None,
            last_frame: None,
        }
    }

    pub const fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub const fn is_sleeping(&self) -> bool {
        self.sleeping
    }

    pub const fn refresh_count(&self) -> u32 {
        self.refresh_count
    }

    pub const fn last_refresh_mode(&self) -> Option<DisplayRefreshMode> {
        self.last_refresh_mode
    }

    pub fn last_frame(&self) -> Option<&ScreenBuffer> {
        self.last_frame.as_ref()
    }
}

impl EpaperDisplay for MockDisplay {
    fn init(&mut self) -> Result<(), DisplayError> {
        self.initialized = true;
        self.sleeping = false;
        Ok(())
    }

    fn refresh(
        &mut self,
        buffer: &ScreenBuffer,
        mode: DisplayRefreshMode,
    ) -> Result<(), DisplayError> {
        if !self.initialized {
            return Err(DisplayError::NotInitialized);
        }

        self.sleeping = false;
        self.refresh_count = self.refresh_count.saturating_add(1);
        self.last_refresh_mode = Some(mode);
        self.last_frame = Some(buffer.clone());
        Ok(())
    }

    fn sleep(&mut self) -> Result<(), DisplayError> {
        if !self.initialized {
            return Err(DisplayError::NotInitialized);
        }

        self.sleeping = true;
        Ok(())
    }
}
