use iced::widget::{
    button, column, container, horizontal_space, row, scrollable, text, vertical_space,
};
use iced::{Background, Border, Center, Color, Element, Fill, Font, Length, Task, Theme};

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

#[cfg(target_os = "linux")]
compile_error!("Linux is not supported in this context.");

pub fn main() -> iced::Result {
    iced::application("DELTASAVER", Deltasaver::update, Deltasaver::view)
        .theme(|_| Theme::Dark)
        .font(include_bytes!("../fonts/DTM-Mono.otf").as_slice())
        .default_font(Font::with_name("Determination Mono"))
        .run_with(Deltasaver::new)
}

// tricky tony better not pull a trick
const CHAPTER_COUNT: Chapter = 7;

const BUILTIN_SLOT_MAX_INDEX: Slot = 2;

const SPACING0_5: f32 = 0.5 * SPACING;
const SPACING: f32 = 8.0;
const SPACING1_5: f32 = 1.5 * SPACING;
const SPACING2: f32 = 2.0 * SPACING;

const TABLE_COLUMN_HEADER_SIZE: f32 = 24.0;
const BUTTON_SIZE: f32 = 12.0;

#[derive(Debug, Clone)]
struct SaveFile {
    path: PathBuf,
    chapter: u8,
    slot: u8,
    hash: Option<String>,
    modified: Option<SystemTime>,
    is_local: bool,
}

impl SaveFile {
    fn display_name(&self) -> String {
        if self.is_local {
            format!(
                "Chapter {}, Slot {} ({})",
                self.chapter,
                self.slot + 1,
                self.hash.as_ref().map(|h| &h[..8]).unwrap_or("local")
            )
        } else {
            format!("Chapter {}, Slot {}", self.chapter, self.slot + 1)
        }
    }
}

type Chapter = u8;
type Slot = u8;

struct Deltasaver {
    deltarune_saves_directory: PathBuf,
    local_saves_directory: PathBuf,
    game_saves: HashMap<(Chapter, Slot), SaveFile>,
    local_saves: Vec<SaveFile>,
    loading: bool,
}

#[derive(Debug, Clone)]
enum Message {
    SavesLoaded(Result<(HashMap<(Chapter, Slot), SaveFile>, Vec<SaveFile>), LoadError>),
    RefreshSaves,
    BackupSave(Chapter, Slot),
    /// local save path, target chapter, slot
    RestoreSave(PathBuf, Chapter, Slot),
    DeleteLocalSave(PathBuf),
}

#[derive(Debug, Clone)]
enum LoadError {
    IoError(()),
}

impl Deltasaver {
    fn new() -> (Self, Task<Message>) {
        let app_data_directory = dirs::data_local_dir()
            .expect("You have no local storage directory. Are you sure you downloaded DELTARUNE?");

        let deltarune_saves_directory = if cfg!(target_os = "windows") {
            app_data_directory.join("DELTARUNE")
        } else if cfg!(target_os = "macos") {
            app_data_directory.join("com.tobyfox.deltarune")
        } else {
            unreachable!("Unsupported OS");
        };

        let local_saves_directory = app_data_directory.join("DELTASAVER");

        if !local_saves_directory.exists() {
            let _ = fs::create_dir_all(&local_saves_directory);
        }

        let app = Self {
            deltarune_saves_directory: deltarune_saves_directory.clone(),
            local_saves_directory: local_saves_directory.clone(),
            game_saves: HashMap::new(),
            local_saves: Vec::new(),
            loading: true,
        };

        (
            app,
            Task::perform(
                load_saves(deltarune_saves_directory, local_saves_directory),
                Message::SavesLoaded,
            ),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SavesLoaded(result) => {
                self.loading = false;
                match result {
                    Ok((game_saves, local_saves)) => {
                        self.game_saves = game_saves;
                        self.local_saves = local_saves;
                    }
                    Err(_) => {
                        // Handle error - maybe show a message to user
                    }
                }
                Task::none()
            }
            Message::RefreshSaves => {
                self.loading = true;
                Task::perform(
                    load_saves(
                        self.deltarune_saves_directory.clone(),
                        self.local_saves_directory.clone(),
                    ),
                    Message::SavesLoaded,
                )
            }
            Message::BackupSave(chapter, slot) => {
                if let Some(save) = self.game_saves.get(&(chapter, slot)) {
                    Task::perform(
                        backup_save(
                            save.path.clone(),
                            self.local_saves_directory.clone(),
                            chapter,
                            slot,
                        ),
                        |_| Message::RefreshSaves,
                    )
                } else {
                    Task::none()
                }
            }
            Message::RestoreSave(local_path, chapter, slot) => Task::perform(
                restore_save(
                    local_path,
                    self.deltarune_saves_directory.clone(),
                    chapter,
                    slot,
                ),
                |_| Message::RefreshSaves,
            ),
            Message::DeleteLocalSave(path) => {
                Task::perform(delete_local_save(path), |_| Message::RefreshSaves)
            }
        }
    }

    fn view(&self) -> Element<Message> {
        if self.loading {
            return container(text("Loading saves..."))
                .center_x(Fill)
                .center_y(Fill)
                .into();
        }

        let game_saves_column = self.create_game_saves_column();
        let local_saves_column = self.create_local_saves_column();

        container(
            row![
                game_saves_column,
                vertical_space().width(SPACING2),
                local_saves_column
            ]
            .height(Fill),
        )
        .padding(SPACING1_5)
        .height(Fill)
        .into()
    }

    fn create_game_saves_column(&self) -> Element<Message> {
        let mut content = column![text("Game Saves").size(TABLE_COLUMN_HEADER_SIZE)].spacing(5);

        for chapter in 1..=CHAPTER_COUNT {
            let chapter_title = text(format!("Chapter {}", chapter)).size(SPACING2);
            let mut slots_cell = column![].spacing(SPACING);

            for slot in 0..=BUILTIN_SLOT_MAX_INDEX {
                let slot_content = if let Some(save) = self.game_saves.get(&(chapter, slot)) {
                    column![
                        button(text(format!("Slot {}", slot + 1)).size(BUTTON_SIZE))
                            .on_press(Message::BackupSave(chapter, slot))
                            .width(Length::Fixed(80.0)),
                        vertical_space().height(SPACING),
                        text(format!(
                            "Modified: {}",
                            save.modified
                                .map(|t| format!("{:?}", t))
                                .unwrap_or("Unknown".to_string())
                        ))
                        .size(10)
                    ]
                } else {
                    column![
                        button(text(format!("Slot {}", slot + 1)).size(BUTTON_SIZE))
                            .width(Length::Fixed(80.0)),
                        text("Empty").size(10)
                    ]
                };

                slots_cell = slots_cell.push(
                    container(slot_content.width(Length::Fill))
                        .padding(SPACING)
                        .style(textbox_style),
                );
            }

            content = content.push(chapter_title).push(slots_cell);
        }

        container(
            scrollable(row![
                content,
                horizontal_space().width(Length::Fixed(SPACING2))
            ])
            .height(Fill)
            .width(Fill),
        )
        .padding(SPACING1_5)
        .style(column_style)
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn create_local_saves_column(&self) -> Element<Message> {
        let mut content =
            column![text("Local Saves").size(TABLE_COLUMN_HEADER_SIZE)].spacing(SPACING);

        let mut saves_by_chapter: HashMap<Chapter, Vec<&SaveFile>> = HashMap::new();
        for save in &self.local_saves {
            saves_by_chapter
                .entry(save.chapter)
                .or_insert_with(Vec::new)
                .push(save);
        }

        for chapter in 1..=CHAPTER_COUNT {
            let chapter_title = text(format!("Chapter {}", chapter)).size(16);

            if let Some(saves) = saves_by_chapter.get(&chapter) {
                let mut slots_by_slot: HashMap<Slot, Vec<&SaveFile>> = HashMap::new();
                for save in saves {
                    slots_by_slot
                        .entry(save.slot)
                        .or_insert_with(Vec::new)
                        .push(save);
                }

                let mut chapter_content = column![chapter_title].spacing(SPACING);

                for slot in 0..=BUILTIN_SLOT_MAX_INDEX {
                    if let Some(slot_saves) = slots_by_slot.get(&slot) {
                        let slot_title = text(format!("Slot {}", slot + 1)).size(14);
                        let mut slot_cell = column![].spacing(SPACING);

                        for save in slot_saves {
                            let save_content = column![
                                button(text(save.display_name()).size(10))
                                    .on_press(Message::RestoreSave(
                                        save.path.clone(),
                                        chapter,
                                        slot
                                    ))
                                    .width(Length::Fixed(120.0)),
                                button(text("Delete").size(10))
                                    .on_press(Message::DeleteLocalSave(save.path.clone()))
                                    .width(Length::Fixed(120.0)),
                                vertical_space().height(SPACING),
                                text(format!(
                                    "Modified: {}",
                                    save.modified
                                        .map(|t| format!("{:?}", t))
                                        .unwrap_or("Unknown".to_string())
                                ))
                                .size(8)
                            ]
                            .spacing(2);

                            slot_cell = slot_cell.push(
                                container(save_content.width(Length::Fill))
                                    .padding(SPACING)
                                    .style(textbox_style),
                            );
                        }

                        chapter_content = chapter_content.push(slot_title).push(slot_cell);
                    }
                }

                content = content.push(chapter_content);
            } else {
                content = content.push(chapter_title).push(text("No saves").size(12));
            }
        }

        container(
            scrollable(row![
                content,
                horizontal_space().width(Length::Fixed(SPACING2))
            ])
            .height(Fill)
            .width(Fill),
        )
        .padding(SPACING1_5)
        .style(column_style)
        .width(Fill)
        .height(Fill)
        .into()
    }
}

fn container_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
        border: Border {
            radius: SPACING0_5.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn textbox_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::BLACK)),
        border: Border {
            color: Color::WHITE,
            width: SPACING0_5,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn column_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(0.05, 0.05, 0.05))),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

async fn load_saves(
    deltarune_directory: PathBuf,
    local_directory: PathBuf,
) -> Result<(HashMap<(Chapter, Slot), SaveFile>, Vec<SaveFile>), LoadError> {
    let mut game_saves = HashMap::new();
    let mut local_saves = Vec::new();

    if deltarune_directory.exists() {
        let entries = fs::read_dir(&deltarune_directory).map_err(|e| LoadError::IoError(()))?;

        for entry in entries {
            let entry = entry.map_err(|e| LoadError::IoError(()))?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                println!("Found game file: {}", filename);
                if let Some((chapter, slot)) = parse_save_filename(filename) {
                    println!("Parsed as chapter {} slot {}", chapter, slot);
                    let modified = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
                    let save = SaveFile {
                        path: path.clone(),
                        chapter,
                        slot,
                        hash: None,
                        modified,
                        is_local: false,
                    };
                    game_saves.insert((chapter, slot), save);
                } else {
                    println!("Could not parse filename: {}", filename);
                }
            }
        }
    }

    // Load local saves
    if local_directory.exists() {
        let entries = fs::read_dir(&local_directory).map_err(|e| LoadError::IoError(()))?;

        for entry in entries {
            let entry = entry.map_err(|e| LoadError::IoError(()))?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if let Some((chapter, slot, hash)) = parse_local_save_filename(filename) {
                    let modified = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
                    let save = SaveFile {
                        path: path.clone(),
                        chapter,
                        slot,
                        hash: Some(hash),
                        modified,
                        is_local: true,
                    };
                    local_saves.push(save);
                }
            }
        }
    }

    Ok((game_saves, local_saves))
}

fn parse_save_filename(filename: &str) -> Option<(Chapter, Slot)> {
    println!("Parsing filename: {}", filename);
    if filename.starts_with("filech") {
        let parts: Vec<&str> = filename[6..].split('_').collect();
        println!("Parts: {:?}", parts);
        if parts.len() == 2 {
            if let (Ok(chapter), Ok(slot)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>()) {
                if slot <= 2 {
                    println!("Successfully parsed: chapter {}, slot {}", chapter, slot);
                    return Some((chapter, slot));
                } else {
                    println!("Ignoring slot {} (only 0-2 are save slots)", slot);
                }
            }
        }
    }
    println!("Failed to parse filename: {}", filename);
    None
}

fn parse_local_save_filename(filename: &str) -> Option<(Chapter, Slot, String)> {
    if filename.starts_with("filech") {
        let parts: Vec<&str> = filename[6..].split('_').collect();
        if parts.len() >= 3 {
            if let (Ok(chapter), Ok(slot)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>()) {
                if slot <= 2 {
                    return Some((chapter, slot, parts[2].to_string()));
                }
            }
        }
    }
    None
}

async fn backup_save(
    source_path: PathBuf,
    local_directory: PathBuf,
    chapter: Chapter,
    slot: Slot,
) -> Result<(), io::Error> {
    let contents = fs::read(&source_path)?;
    let hash = format!("{:x}", Sha256::digest(&contents));
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let filename = format!(
        "filech{}_{}_{}_{}_{}",
        chapter,
        slot,
        hash,
        now.as_secs(),
        now.subsec_nanos()
    );
    let dest_path = local_directory.join(filename);
    fs::write(dest_path, contents)?;
    Ok(())
}

async fn restore_save(
    local_path: PathBuf,
    deltarune_directory: PathBuf,
    chapter: Chapter,
    slot: Slot,
) -> Result<(), io::Error> {
    let contents = fs::read(&local_path)?;
    let dest_path = deltarune_directory.join(format!("filech{}_{}", chapter, slot));
    fs::write(dest_path, contents)?;
    Ok(())
}

async fn delete_local_save(path: PathBuf) -> Result<(), io::Error> {
    fs::remove_file(path)
}
