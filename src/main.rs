use iced::{
    button, scrollable, Button, Column, Command, Element, Scrollable, Text,
    Application, Settings, Length, Row, Space
};
use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    time::{Instant, Duration},
};

#[derive(Debug, Clone)]
pub enum Message {
    FileClicked(PathBuf),
    Refresh,
    GoUp,
    DriveSelected(PathBuf),
}

#[derive(Default)]
struct FileExplorer {
    path: PathBuf,
    files: Vec<PathBuf>,
    scroll: scrollable::State,
    drives_scroll: scrollable::State,
    refresh_button: button::State,
    up_button: button::State,
    drive_button: button::State,
    file_buttons: Vec<button::State>,
    drives: Vec<PathBuf>,
    drive_buttons: Vec<button::State>,
    show_drives: bool,
    last_click_time: Option<Instant>,  // Track the last click time
}

impl FileExplorer {
    fn list_files(&mut self) -> Command<Message> {
        let files = self.list_files_in_directory(&self.path);
        self.files = files;
        Command::none()
    }

    fn list_files_in_directory(&self, path: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        
        if path.parent().is_some() {
            files.push(PathBuf::from(".."));
        }
        
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                files.push(entry.path());
            }
        }
        
        files.sort_by(|a, b| {
            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();
            if a_is_dir && !b_is_dir {
                std::cmp::Ordering::Less
            } else if !a_is_dir && b_is_dir {
                std::cmp::Ordering::Greater
            } else {
                a.file_name().cmp(&b.file_name())
            }
        });
        
        files
    }
    
    fn get_available_drives() -> Vec<PathBuf> {
        if cfg!(windows) {
            (b'A'..=b'Z')
                .map(|c| format!("{}:", c as char))
                .map(PathBuf::from)
                .filter(|drive| drive.exists())
                .collect()
        } else {
            vec![PathBuf::from("/")]
        }
    }

    fn open_file(&self, file_path: &Path) {
        if cfg!(windows) {
            if let Some(valid_path) = file_path.to_str() {
                if let Err(err) = ProcessCommand::new("cmd")
                    .args(&["/C", "start", valid_path])
                    .spawn()
                {
                    eprintln!("Failed to open file (Windows): {}", err);
                }
            }
        } else {
            if let Err(err) = ProcessCommand::new("xdg-open")
                .arg(file_path)  // Use to_string_lossy for safe conversion
                .spawn()
            {
                eprintln!("Failed to open file (Linux/macOS): {}", err);
            }
        }
    }
}

impl Application for FileExplorer {
    type Message = Message;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (FileExplorer, Command<Message>) {
        let drives = FileExplorer::get_available_drives();
        let drive_buttons = drives.iter().map(|_| button::State::new()).collect();
        
        let mut explorer = FileExplorer {
            path: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            drives,
            drive_buttons,
            show_drives: false,
            last_click_time: None,  // Initialize with no click
            ..FileExplorer::default()
        };
        explorer.list_files();
        (explorer, Command::none())
    }

    fn title(&self) -> String {
        String::from("File Explorer")
    }

    fn update(
        &mut self,
        message: Self::Message,
        _clipboard: &mut iced::Clipboard,
    ) -> Command<Message> {
        match message {
            Message::FileClicked(path) => {
                let now = Instant::now();
                
                // If we had a previous click and it's within 500ms, consider it a double-click
                if let Some(last_click) = self.last_click_time {
                    if now.duration_since(last_click) < Duration::from_millis(500) {
                        // Double-click detected, open the file
                        if path.is_file() {
                            self.open_file(&path);
                        }
                    }
                }

                // Update the last click time
                self.last_click_time = Some(now);

                // Handle file/directory navigation
                let target_path = if path == PathBuf::from("..") {
                    self.path.parent().map_or(self.path.clone(), |p| p.to_path_buf())
                } else if path.is_relative() {
                    self.path.join(&path)
                } else {
                    path
                };

                if target_path.is_dir() {
                    self.path = target_path;
                    self.show_drives = false;
                    self.list_files()
                } else {
                    println!("File selected: {:?}", target_path);
                    Command::none()  // No further action if it’s just a click (not a double-click)
                }
            }
            Message::Refresh => self.list_files(),
            Message::GoUp => {
                if let Some(parent) = self.path.parent() {
                    self.path = parent.to_path_buf();
                    self.show_drives = false;
                    self.list_files()
                } else {
                    self.show_drives = true;
                    Command::none()
                }
            }
            Message::DriveSelected(drive_path) => {
                self.path = drive_path;
                self.show_drives = false;
                self.list_files()
            }
        }
    }

    fn view(&mut self) -> Element<Message> {
        // Main column with spacing and padding
        let mut column = Column::new().spacing(10).padding(10);

        // 1. Show current directory at top
        column = column.push(
            Text::new(format!("Directory: {}", self.path.display()))
            .size(16)
        );

        // Top buttons row
        let mut top_row = Row::new().spacing(10);
        top_row = top_row.push(
            Button::new(&mut self.refresh_button, Text::new("Refresh"))
                .on_press(Message::Refresh)
                .padding(5),
        );
        top_row = top_row.push(
            Button::new(&mut self.up_button, Text::new("Go Up"))
                .on_press(Message::GoUp)
                .padding(5),
        );
        
        // Drive button (Windows only)
        if cfg!(windows) {
            top_row = top_row.push(
                Button::new(&mut self.drive_button, Text::new("Drives"))
                    .on_press(Message::DriveSelected(PathBuf::from("C:\\")))  // or any default drive
                    .padding(5),
            );
        }
        
        column = column.push(top_row);

        // Drive selection (if shown)
        if self.show_drives {
            self.drive_buttons
                .resize_with(self.drives.len(), button::State::new);

            column = column.push(Text::new("Select Drive:").size(14));
            
            let mut drives_row = Row::new().spacing(5);
            let drive_buttons = self.drive_buttons.iter_mut();
            
            for (drive, btn_state) in self.drives.iter().zip(drive_buttons) {
                let drive_name = drive.display().to_string();
                let is_current = self.path.starts_with(drive);
                
                let button = Button::new(
                    btn_state, 
                    Text::new(if is_current {
                        format!("[{}]", drive_name)
                    } else {
                        drive_name.clone()
                    })
                )
                .on_press(Message::DriveSelected(drive.clone()))
                .padding(5);
                
                drives_row = drives_row.push(button);
            }
            
            column = column.push(                        
                        Scrollable::new(&mut self.drives_scroll)
                            .push(drives_row)
                            .height(Length::Units(40))
                            .width(Length::Fill)
                    );
                    
                    column = column.push(Space::with_height(Length::Units(10)));
                }

                // Files list with proper spacing
                let mut files_column = Column::new().spacing(5);
                self.file_buttons
                    .resize_with(self.files.len(), button::State::new);

                for (file, btn_state) in self.files.iter().zip(self.file_buttons.iter_mut()) {
                    let display_name = if file == &PathBuf::from("..") {
                        ".. (parent)".to_string()
                    } else {
                        file.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string()
                    };

                    // 2. Show files/directories with icons
                    let prefix = if file.is_dir() { 
                        Text::new("📁 ")  // Folder icon
                    } else {
                        Text::new("📄 ")  // File icon
                    };

                    let full_text = Row::new()
                        .push(prefix)
                        .push(Text::new(display_name));

                    // 3. Navigate into directories by clicking
                    let button = Button::new(btn_state, full_text)
                        .on_press(Message::FileClicked(file.clone()))
                        .padding(5);

                    files_column = files_column.push(button);
                }

                column = column.push(
                    Scrollable::new(&mut self.scroll)
                        .push(files_column)
                        .height(Length::Fill)
                );

                column.into()
            }
        }

fn main() -> iced::Result {
    FileExplorer::run(Settings::default())
}
