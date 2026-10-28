/// Ctrl-C reaching the program as a key, rather than ending it, for as long as this lives.
/// The terminal is set back as it was when dropped.
pub struct AsKey(Saved);

impl AsKey {
    /// `None` when the terminal would not take the setting; Ctrl-C then still ends the
    /// program between two key reads.
    pub fn hold() -> Option<Self> {
        hold().map(AsKey)
    }
}

impl Drop for AsKey {
    fn drop(&mut self) {
        restore(&self.0);
    }
}

#[cfg(unix)]
struct Saved(libc::termios);

#[cfg(unix)]
fn hold() -> Option<Saved> {
    // SAFETY: tcgetattr fills the termios it is given; tcsetattr only reads it.
    unsafe {
        let mut termios = std::mem::zeroed::<libc::termios>();
        if libc::tcgetattr(libc::STDIN_FILENO, &mut termios) != 0 {
            return None;
        }
        let saved = termios;
        termios.c_lflag &= !libc::ISIG;
        if libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &termios) != 0 {
            return None;
        }
        Some(Saved(saved))
    }
}

#[cfg(unix)]
fn restore(saved: &Saved) {
    // SAFETY: tcsetattr only reads the termios it is given.
    unsafe {
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &saved.0);
    }
}

#[cfg(windows)]
struct Saved(windows_sys::Win32::System::Console::CONSOLE_MODE);

#[cfg(windows)]
fn hold() -> Option<Saved> {
    use windows_sys::Win32::System::Console::{
        ENABLE_PROCESSED_INPUT, GetConsoleMode, GetStdHandle, STD_INPUT_HANDLE, SetConsoleMode,
    };
    // SAFETY: GetConsoleMode writes the mode it is given; the handle is the process's own.
    unsafe {
        let input = GetStdHandle(STD_INPUT_HANDLE);
        let mut mode = 0;
        if GetConsoleMode(input, &mut mode) == 0 {
            return None;
        }
        if SetConsoleMode(input, mode & !ENABLE_PROCESSED_INPUT) == 0 {
            return None;
        }
        Some(Saved(mode))
    }
}

#[cfg(windows)]
fn restore(saved: &Saved) {
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE, SetConsoleMode};
    // SAFETY: the handle is the process's own console input.
    unsafe {
        SetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), saved.0);
    }
}
